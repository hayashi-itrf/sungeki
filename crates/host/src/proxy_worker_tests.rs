use super::super::{
    proxy_record::{Input, State, decode},
    proxy_worker,
};
use crate::sessions::Sessions;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use sungeki_project::{Asset, AssetLocator, AssetProbe, JobPhase};
use sungeki_protocol::{SessionRequest, SessionResponse};
use sungeki_storage::{JobKind, StoredJob};

async fn start(
    shared: &Arc<Mutex<Sessions>>,
    session: &str,
    input: &Input,
) -> (String, Arc<AtomicBool>) {
    let id = uuid::Uuid::new_v4().to_string();
    let flag = Arc::new(AtomicBool::new(false));
    let runtime = {
        let mut sessions = shared.lock().unwrap();
        sessions
            .store
            .create_job(&StoredJob {
                id: id.clone(),
                session_id: session.into(),
                kind: JobKind::Proxy,
                version: 0,
                input: serde_json::to_vec(input).unwrap(),
                state: serde_json::to_vec(&State::queued()).unwrap(),
            })
            .unwrap();
        let runtime = sessions.render_runtime.clone();
        runtime
            .flags
            .lock()
            .unwrap()
            .insert(id.clone(), flag.clone());
        runtime.active.fetch_add(1, Ordering::SeqCst);
        runtime
    };
    proxy_worker::launch(shared.clone(), runtime, id.clone(), flag.clone());
    (id, flag)
}
async fn finished(shared: &Arc<Mutex<Sessions>>, id: &str) -> State {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let sessions = shared.lock().unwrap();
        let job = sessions.store.load_job(id).unwrap().unwrap();
        let (_, state) = decode(&job).unwrap();
        let settled = !sessions
            .render_runtime
            .flags
            .lock()
            .unwrap()
            .contains_key(id);
        drop(sessions);
        if state.phase.is_terminal() && settled {
            return state;
        }
        assert!(Instant::now() < deadline, "proxy job did not settle");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

pub(super) async fn run(
    root: &Path,
    source_path: &Path,
    manifest: &sungeki_assets::proxy_store::ProxyManifest,
    ffmpeg: &Path,
    ffprobe: &Path,
    work: &Path,
    cache: &Path,
) {
    let mut sessions = Sessions::open(&root.join("proxy-jobs.db")).unwrap();
    let SessionResponse::Snapshot { snapshot } = sessions
        .request(SessionRequest::Create {
            title: "軽量動画ジョブ検証".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let asset_id = uuid::Uuid::new_v4().to_string();
    sessions
        .request(SessionRequest::Edit {
            session_id: snapshot.session_id.clone(),
            edit: sungeki_editor::EditRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: 0,
                action: sungeki_editor::EditAction::Apply {
                    commands: vec![sungeki_editor::Command::RegisterAsset {
                        asset: Asset {
                            id: asset_id.clone(),
                            name: "実動画原本".into(),
                            credit: "自作検査模様".into(),
                            source: None,
                            locator: AssetLocator::Absolute {
                                path: source_path.to_str().unwrap().into(),
                            },
                            probe: AssetProbe::Video(manifest.source.clone()),
                            content_hash: Some(manifest.source.sha256.clone()),
                        },
                    }],
                },
            },
        })
        .unwrap();
    let (_, current) = sessions.load(&snapshot.session_id).unwrap();
    let project = current.history().project().clone();
    let input = Input {
        source_revision: current.history().revision(),
        content_hash: sungeki_storage::project_hash(&project).unwrap(),
        project,
        asset_ids: vec![asset_id],
        max_edge: 160,
        encoding: sungeki_media::video_probe::frames::proxy::ProxyEncoding::Qtrle,
        encoder_sha256: manifest.encoder_sha256.clone(),
        ffmpeg: ffmpeg.into(),
        ffprobe: ffprobe.into(),
        work_root: work.into(),
        cache_root: cache.into(),
    };
    let before = sessions.store.load(&snapshot.session_id).unwrap().unwrap();
    let runtime = sessions.render_runtime.clone();
    let shared = Arc::new(Mutex::new(sessions));
    let original_key = manifest.key().unwrap();
    let cached_path = cache.join(original_key.as_str()).join("video.mov");
    let stamp = fs::metadata(&cached_path).unwrap().modified().unwrap();
    let (id, _) = start(&shared, &snapshot.session_id, &input).await;
    let complete = finished(&shared, &id).await;
    assert_eq!(complete.phase, JobPhase::Succeeded, "{:?}", complete.error);
    assert_eq!(complete.completed.len(), 1);
    assert_eq!(complete.completed[0].key, original_key.as_str());
    assert_eq!(
        fs::metadata(&cached_path).unwrap().modified().unwrap(),
        stamp
    );

    // A real encoder wrapper pauses the first attempt; a foreground request must
    // reap that attempt, retain the job, and permit a subsequent real conversion.
    use std::os::unix::fs::PermissionsExt;
    let marker = root.join("preempt.ready");
    let wrapper = root.join("encoder.sh");
    let quote = |p: &Path| p.to_str().unwrap().replace('\'', "'\\''");
    fs::write(&wrapper, format!("#!/bin/sh\nif [ ! -e '{}' ]; then\nprintf ready > '{}'\nexec /bin/sleep 30\nfi\nexec '{}' \"$@\"\n", quote(&marker), quote(&marker), quote(ffmpeg))).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let mut retry = input.clone();
    retry.max_edge = 128;
    retry.ffmpeg = wrapper.clone();
    retry.encoder_sha256 = format!("{:x}", Sha256::digest(fs::read(&wrapper).unwrap()));
    let (id, flag) = start(&shared, &snapshot.session_id, &retry).await;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !marker.exists() {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    {
        let sessions = shared.lock().unwrap();
        let job = sessions.store.load_job(&id).unwrap().unwrap();
        let (_, state) = decode(&job).unwrap();
        assert_eq!(state.progress.unwrap().stage, "軽量動画を生成・検証中");
    }
    let foreground = tokio::time::timeout(
        Duration::from_secs(3),
        runtime.slots.clone().acquire_owned(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(!flag.load(Ordering::Acquire));
    assert_eq!(fs::read_dir(work).unwrap().count(), 0);
    drop(foreground);
    let retried = finished(&shared, &id).await;
    assert_eq!(retried.phase, JobPhase::Succeeded, "{:?}", retried.error);
    assert_eq!(retried.completed.len(), 1);
    assert_eq!(
        retried.completed[0].key,
        retry.key(&retry.asset_ids[0]).unwrap()
    );

    let held = runtime.slots.clone().acquire_owned().await.unwrap();
    let (id, flag) = start(&shared, &snapshot.session_id, &input).await;
    flag.store(true, Ordering::Release);
    let cancelled = finished(&shared, &id).await;
    assert_eq!(cancelled.phase, JobPhase::Cancelled);
    assert!(cancelled.completed.is_empty());
    drop(held);
    let cached = fs::read(&cached_path).unwrap();
    fs::write(&cached_path, vec![0; cached.len()]).unwrap();
    let (id, _) = start(&shared, &snapshot.session_id, &input).await;
    let rejected = finished(&shared, &id).await;
    assert_eq!(rejected.phase, JobPhase::Failed);
    assert!(rejected.completed.is_empty());
    assert!(rejected.error.is_some());
    assert_eq!(
        fs::read(&cached_path).unwrap(),
        vec![0; cached.len()],
        "corrupted existing cache must not be overwritten"
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load(&snapshot.session_id)
            .unwrap()
            .unwrap(),
        before
    );
    assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
    assert_eq!(fs::read_dir(work).unwrap().count(), 0);
}
