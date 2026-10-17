use crate::{
    image_probe::video::{
        proxy_record::{Input, State, decode, invalid},
        proxy_worker,
    },
    sessions::{Sessions, storage_error},
};
use std::{
    collections::HashSet,
    fs,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use sungeki_project::{AssetLocator, AssetProbe};
use sungeki_protocol::*;
use sungeki_storage::{JobKind, StoredJob};
fn validation(message: &str) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message)
}
fn replay(
    sessions: &Sessions,
    args: &GenerateProxiesArgs,
) -> Result<Option<SessionResponse>, ApiError> {
    let Some(job) = sessions
        .store
        .load_job(&args.job_id)
        .map_err(storage_error)?
    else {
        return Ok(None);
    };
    if job.kind != JobKind::Proxy {
        return Err(ApiError::new(ErrorCode::Conflict, "別種類のジョブIDです"));
    }
    let (input, _) = decode(&job)?;
    if job.session_id != args.session_id
        || input.source_revision != args.expected_revision
        || input.asset_ids != args.asset_ids
        || input.max_edge != args.preset.max_edge()
    {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "別の要求に使われているジョブIDです",
        ));
    }
    super::result(sessions, &args.job_id).map(Some)
}
fn directory(root: &Path, name: &str) -> Result<PathBuf, ApiError> {
    let path = root.join(name);
    match fs::DirBuilder::new().mode(0o700).create(&path) {
        Ok(()) => {}
        Err(e)
            if e.kind() == std::io::ErrorKind::AlreadyExists
                && fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) => {}
        Err(e) => return Err(invalid(e)),
    }
    Ok(path)
}
fn executables(
    saved: &crate::runtime_settings::Settings,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<(PathBuf, PathBuf), ApiError> {
    let path = |name: &str, value: Option<&Path>| {
        crate::runtime_settings::file(env(name).map(PathBuf::from), value, name)?
            .canonicalize()
            .map_err(invalid)
    };
    Ok((
        path("SUNGEKI_RENDER_FFMPEG", saved.ffmpeg.as_deref())?,
        path("SUNGEKI_RENDER_FFPROBE", saved.ffprobe.as_deref())?,
    ))
}
pub(super) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: GenerateProxiesArgs,
) -> Result<SessionResponse, ApiError> {
    if uuid::Uuid::parse_str(&args.job_id)
        .ok()
        .is_none_or(|id| id.to_string() != args.job_id)
        || args.asset_ids.is_empty()
        || args.asset_ids.len() > 64
        || args.asset_ids.iter().collect::<HashSet<_>>().len() != args.asset_ids.len()
    {
        return Err(validation(
            "標準UUIDのjob_idと、重複しない1〜64件の動画素材を指定してください",
        ));
    }
    let (project, database, runtime) = {
        let sessions = shared.lock().map_err(invalid)?;
        if let Some(response) = replay(&sessions, &args)? {
            return Ok(response);
        }
        let (_, current) = sessions.load(&args.session_id)?;
        if current.history().revision() != args.expected_revision {
            return Err(ApiError::new(ErrorCode::Conflict, "作品が更新されています"));
        }
        let project = current.history().project().clone();
        for id in &args.asset_ids {
            let asset = project
                .assets
                .iter()
                .find(|a| &a.id == id)
                .ok_or_else(|| validation("対象素材がありません"))?;
            if !matches!(asset.probe, AssetProbe::Video(_))
                || !matches!(asset.locator, AssetLocator::Absolute { .. })
            {
                return Err(validation("検査済み動画の絶対パスが必要です"));
            }
        }
        (
            project,
            sessions.review_database_path.clone(),
            sessions.render_runtime.clone(),
        )
    };
    // Hashing runtime files and preparing directories never hold the editor lock.
    let saved = crate::runtime_settings::load(&database)?;
    let (ffmpeg, ffprobe) = executables(&saved, |name| std::env::var_os(name))?;
    let (encoder_sha256, _) = sungeki_media::source_identity(&ffmpeg, 256 * 1024 * 1024, || {
        runtime.stopping.load(Ordering::Acquire)
    })
    .map_err(invalid)?;
    let root = crate::render_location::root(&database)?;
    let input = Input {
        source_revision: args.expected_revision,
        content_hash: sungeki_storage::project_hash(&project).map_err(invalid)?,
        project,
        asset_ids: args.asset_ids.clone(),
        max_edge: args.preset.max_edge(),
        encoding: sungeki_media::video_probe::frames::proxy::ProxyEncoding::Qtrle,
        encoder_sha256,
        ffmpeg,
        ffprobe,
        work_root: directory(&root, "proxy-work")?,
        cache_root: directory(&root, "proxy-cache")?,
    };
    let job = StoredJob {
        id: args.job_id.clone(),
        session_id: args.session_id.clone(),
        kind: JobKind::Proxy,
        version: 0,
        input: serde_json::to_vec(&input).map_err(invalid)?,
        state: serde_json::to_vec(&State::queued()).map_err(invalid)?,
    };
    decode(&job)?;
    tokio::runtime::Handle::try_current().map_err(|_| {
        ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "ジョブの実行環境がありません",
        )
    })?;
    let mut sessions = shared.lock().map_err(invalid)?;
    if let Some(response) = replay(&sessions, &args)? {
        return Ok(response);
    }
    let (_, current) = sessions.load(&args.session_id)?;
    if current.history().revision() != args.expected_revision
        || sungeki_storage::project_hash(current.history().project()).map_err(invalid)?
            != input.content_hash
    {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "準備中に作品が更新されました",
        ));
    }
    let flag = Arc::new(AtomicBool::new(false));
    let mut flags = runtime.flags.lock().map_err(invalid)?;
    if runtime.stopping.load(Ordering::Acquire) {
        return Err(ApiError::new(ErrorCode::Cancelled, "終了処理中です"));
    }
    let mut proxy_active = false;
    for id in flags.keys() {
        proxy_active |= sessions
            .store
            .load_job(id)
            .map_err(storage_error)?
            .is_some_and(|j| j.kind == JobKind::Proxy);
    }
    if runtime.active.load(Ordering::SeqCst) >= 3 || proxy_active {
        let mut error = ApiError::new(
            ErrorCode::Conflict,
            "他の素材処理を待ってから軽量動画を生成してください",
        );
        error.retryable = true;
        return Err(error);
    }
    sessions.store.create_job(&job).map_err(storage_error)?;
    flags.insert(job.id.clone(), flag.clone());
    runtime.active.fetch_add(1, Ordering::SeqCst);
    drop(flags);
    let response = super::result(&sessions, &job.id);
    drop(sessions);
    proxy_worker::launch(shared.clone(), runtime, job.id, flag);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_uses_saved_tools_without_requiring_image_or_font_settings() {
        let fixture = crate::speech_journal_tests::Fixture::new();
        let file = fixture.0.join("tool");
        fs::write(&file, b"tool").unwrap();
        let saved = crate::runtime_settings::Settings {
            format_version: 1,
            ffmpeg: Some(file.clone()),
            ffprobe: Some(file.clone()),
            ..Default::default()
        };
        let expected = file.canonicalize().unwrap();
        assert_eq!(
            executables(&saved, |_| None).unwrap(),
            (expected.clone(), expected)
        );
        let other = fixture.0.join("override");
        fs::write(&other, b"override").unwrap();
        for name in ["SUNGEKI_RENDER_FFMPEG", "SUNGEKI_RENDER_FFPROBE"] {
            let tools = executables(&saved, |key| {
                (key == name).then(|| other.clone().into_os_string())
            })
            .unwrap();
            let selected = if name.ends_with("FFMPEG") {
                tools.0
            } else {
                tools.1
            };
            assert_eq!(selected, other.canonicalize().unwrap());
            assert!(executables(&saved, |key| (key == name).then(|| "missing".into())).is_err());
        }
        assert!(executables(&crate::runtime_settings::Settings::default(), |_| None).is_err());
    }
}
