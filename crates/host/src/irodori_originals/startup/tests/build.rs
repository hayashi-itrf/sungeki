use super::fixture;
use crate::{build_record as build, speech_journal_tests::Fixture};
use serde_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sungeki_project::{AssetLocator, AssetProbe, BuildRecipe, JobPhase, Voice};

#[tokio::test]
async fn build_dispatches_owned_irodori_and_applies_only_through_parent() {
    let (_engine, request, pid) = fixture("speech");
    let store = Fixture::new();
    let mut sessions = store.open();
    let profile: crate::irodori_originals::configuration::Profile = serde_json::from_value(json!({
        "id":"build-irodori","name":"Build", "binding":request.binding,
        "port":request.port,"identity":request.identity,"default_settings":request.settings,
        "model":{"checkpoint":"model","speaker_conditioning":true,"caption_conditioning":false,"sample_rate":48000,"channels":1},
        "voices":[{"id":"none","name":"参照なし","reference":"none","caption":null}]
    })).unwrap();
    let path = sessions
        .review_database_path
        .parent()
        .unwrap()
        .join("irodori.json");
    let configuration =
        serde_json::to_vec(&json!({"format_version":1,"profiles":[profile]})).unwrap();
    std::fs::write(&path, &configuration).unwrap();
    let loaded = build::profiles::LoadedProfile::load(
        &sessions.review_database_path,
        build::profiles::Kind::Irodori,
        "build-irodori",
        &AtomicBool::new(false),
    )
    .unwrap();
    let mut recipe: BuildRecipe = serde_json::from_str(include_str!(
        "../../../../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    for role in &mut recipe.roles {
        role.preset_id = "image-custom-v1".into();
        role.character.voice = Voice::Irodori {
            settings: request.settings.clone(),
            identity: request.identity.clone(),
        };
    }
    let image = store.0.join("source.png");
    std::fs::write(
        &image,
        include_bytes!("../../../../../../tests/fixtures/build-source.png"),
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
        &store.0.join("managed-build.vvm.json"),
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
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    super::gone(&pid);
    let child_id = {
        let sessions = shared.lock().unwrap();
        assert_eq!(
            sessions.store.load(&parent.session_id).unwrap().unwrap(),
            before
        );
        let row = sessions.store.load_job(&parent.id).unwrap().unwrap();
        let id = build::decode(&row).unwrap().1.child_id.unwrap();
        let child = sessions.store.load_job(&id).unwrap().unwrap();
        let (input, state) = crate::speech_journal::decode_input(&child).unwrap();
        assert_eq!(state.phase, JobPhase::Succeeded, "{:?}", state.error);
        assert_eq!(input.parent_job_id(), Some(parent.id.as_str()));
        assert!(matches!(input, crate::speech_journal::Input::Irodori(_)));
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
    // Changed configuration cannot dispatch the next speech or silently adopt it.
    let mut changed: serde_json::Value = serde_json::from_slice(&configuration).unwrap();
    changed["profiles"][0]["name"] = json!("changed after first speech");
    std::fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(build::advance(&shared, &parent.id, None).await.is_err());
    let sessions = shared.lock().unwrap();
    let row = sessions.store.load_job(&parent.id).unwrap().unwrap();
    let id = build::decode(&row).unwrap().1.child_id.unwrap();
    assert!(sessions.store.load_job(&id).unwrap().is_none());
    assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
    assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
    let (plan, _) = build::decode(&row).unwrap();
    let build_runtime = sessions.build_runtime.clone();
    drop(sessions);
    let args = sungeki_protocol::StartBuildVideoArgs {
        wait_ms: None,
        reuse_from: None,
        managed_profiles: plan
            .managed_profiles
            .iter()
            .map(|(id, p)| (id.clone(), p.profile_id.clone()))
            .collect(),
        recipe: plan.recipe,
        job_id: uuid::Uuid::new_v4().to_string(),
        project_path: store
            .0
            .join("public-managed.vvm.json")
            .to_string_lossy()
            .into(),
        base_url: String::new(),
        engine_version: String::new(),
    };
    let response = build::execute(
        &shared,
        sungeki_protocol::BuildVideoRequest::Start { args: args.clone() },
    )
    .unwrap();
    let sungeki_protocol::SessionResponse::BuildVideoJob { job } = response else {
        panic!()
    };
    assert_eq!(job.phase, JobPhase::Queued);
    let saved = shared
        .lock()
        .unwrap()
        .store
        .load_job(&args.job_id)
        .unwrap()
        .unwrap();
    let (saved_plan, _) = build::decode(&saved).unwrap();
    assert!(saved_plan.engine.is_none());
    assert_eq!(
        saved_plan.managed_profiles.len(),
        args.managed_profiles.len()
    );
    assert_ne!(saved_plan.managed_profiles, plan.managed_profiles);
    build::execute(
        &shared,
        sungeki_protocol::BuildVideoRequest::Cancel {
            job_id: args.job_id,
        },
    )
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while build_runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
}
