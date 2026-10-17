//! Shared end-to-end scheduler assertions; engines are local HTTP fixtures.
use crate::{build_record as build, sessions::Sessions};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use sungeki_project::{AssetLocator, AssetProbe, BuildRecipe, JobPhase, Voice};

pub(crate) async fn verify(
    mut sessions: Sessions,
    root: &Path,
    voice: Voice,
    kind: build::profiles::Kind,
    config: serde_json::Value,
    pid: &Path,
    cancel_marker: Option<&Path>,
) {
    let name = match kind {
        build::profiles::Kind::Sbv2 => "sbv2.json",
        build::profiles::Kind::Qwen => "qwen.json",
        build::profiles::Kind::Irodori => unreachable!(),
    };
    let path = sessions.review_database_path.parent().unwrap().join(name);
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let profile_id = config["profiles"][0]["id"].as_str().unwrap();
    let loaded = build::profiles::LoadedProfile::load(
        &sessions.review_database_path,
        kind,
        profile_id,
        &AtomicBool::new(false),
    )
    .unwrap();
    let mut recipe: BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    for role in &mut recipe.roles {
        role.preset_id = "image-custom-v1".into();
        role.character.voice = voice.clone();
    }
    let image = root.join("build-source.png");
    std::fs::write(
        &image,
        include_bytes!("../../../tests/fixtures/build-source.png"),
    )
    .unwrap();
    let probe = sungeki_assets::probe_image(&image, Default::default()).unwrap();
    recipe.assets[0].locator = AssetLocator::Absolute {
        path: image.to_string_lossy().into(),
    };
    recipe.assets[0].content_hash = Some(probe.sha256.clone());
    recipe.assets[0].probe = AssetProbe::Image(probe);
    let bindings = recipe
        .roles
        .iter()
        .map(|r| (r.character.id.clone(), loaded.freeze().unwrap()))
        .collect();
    let parent = build::create_configured(
        &mut sessions,
        &uuid::Uuid::new_v4().to_string(),
        recipe,
        &root.join("managed-build.vvm.json"),
        None,
        bindings,
    )
    .unwrap();
    let runtime = sessions.speech_runtime.clone();
    let shared = Arc::new(Mutex::new(sessions));
    for _ in 0..2 {
        assert_eq!(
            build::advance(&shared, &parent.id, None).await.unwrap(),
            build::Advance::Progress
        );
    }
    let before = shared
        .lock()
        .unwrap()
        .store
        .load(&parent.session_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        build::advance(&shared, &parent.id, None).await.unwrap(),
        build::Advance::Waiting
    );
    if let Some(marker) = cancel_marker {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !marker.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let row = shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap();
        build::cancel_owned(&shared, &row).unwrap();
    }
    tokio::time::timeout(Duration::from_secs(15), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let process: i32 = std::fs::read_to_string(pid).unwrap().parse().unwrap();
    assert_eq!(unsafe { libc::kill(process, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
    let child_id = {
        let mut sessions = shared.lock().unwrap();
        assert_eq!(
            sessions.store.load(&parent.session_id).unwrap().unwrap(),
            before
        );
        let row = sessions.store.load_job(&parent.id).unwrap().unwrap();
        let id = build::decode(&row).unwrap().1.child_id.unwrap();
        let child = sessions.store.load_job(&id).unwrap().unwrap();
        let (input, state) = crate::speech_journal::decode_input(&child).unwrap();
        assert_eq!(input.parent_job_id(), Some(parent.id.as_str()));
        let selected = match &input {
            crate::speech_journal::Input::Sbv2(i) => i.profile_id(),
            crate::speech_journal::Input::Qwen(i) => i.profile_id(),
            _ => panic!("wrong engine"),
        };
        assert_eq!(selected, Some(profile_id));
        assert_eq!(state.execution, crate::speech_state::Execution::Finished);
        assert!(!sessions.speech_resources.has_owner(&id).unwrap());
        assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
        if cancel_marker.is_some() {
            assert_eq!(state.phase, JobPhase::Cancelled, "{:?}", state.error);
            assert!(state.artifact.is_none());
            assert!(build::settle_cancel(&mut sessions, &row).unwrap());
            return;
        }
        assert_eq!(state.phase, JobPhase::Succeeded, "{:?}", state.error);
        id
    };
    assert_eq!(
        build::advance(&shared, &parent.id, None).await.unwrap(),
        build::Advance::Progress
    );
    {
        let sessions = shared.lock().unwrap();
        let (input, _) = crate::speech_journal::decode_input(
            &sessions.store.load_job(&child_id).unwrap().unwrap(),
        )
        .unwrap();
        let (_, current) = sessions.load(&parent.session_id).unwrap();
        assert!(input.applied_audio_matches(current.history().project()));
    }
    let mut changed = config.clone();
    changed["profiles"][0]["name"] = serde_json::json!("changed after dispatch");
    std::fs::write(path, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(build::advance(&shared, &parent.id, None).await.is_err());
    let sessions = shared.lock().unwrap();
    let row = sessions.store.load_job(&parent.id).unwrap().unwrap();
    let id = build::decode(&row).unwrap().1.child_id.unwrap();
    assert!(sessions.store.load_job(&id).unwrap().is_none());
    assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
}
