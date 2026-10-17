use super::*;
use crate::build_record::{SpeechConnection, create_configured, execute};
use std::sync::{Arc, Mutex};
use sungeki_protocol::{BuildVideoRequest, SessionResponse};
#[tokio::test]
async fn source_choices_preserve_order_and_only_expose_applied_audio_records() {
    let (root, shared, parent, child, _) = super::speech_reuse_tests::setup().await;
    let before = shared
        .lock()
        .unwrap()
        .store
        .load_job(&parent)
        .unwrap()
        .unwrap();
    let project_before = shared
        .lock()
        .unwrap()
        .store
        .load(&before.session_id)
        .unwrap()
        .unwrap();
    let SessionResponse::BuildReuseSource { source } = crate::build_record::execute(
        &shared,
        BuildVideoRequest::ReadReuseSource {
            job_id: parent.clone(),
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(source.job_id, parent);
    let (input, _) = decode(&before).unwrap();
    assert_eq!(
        source.base_url,
        input
            .engine
            .as_ref()
            .map(|e| e.endpoint.as_str())
            .unwrap_or("")
    );
    assert_eq!(
        source.engine_version,
        input
            .engine
            .as_ref()
            .map(|e| e.identity.engine_version.as_str())
            .unwrap_or("")
    );
    let expected: Vec<_> = input.recipe.scenes.iter().flat_map(|s| &s.lines).collect();
    assert_eq!(source.lines.len(), expected.len());
    for (index, line) in source.lines.iter().enumerate() {
        assert_eq!(usize::from(line.index), index);
        assert_eq!(line.subtitle_text, expected[index].subtitle_text);
        let role = input
            .recipe
            .roles
            .iter()
            .find(|r| r.role == expected[index].role)
            .unwrap();
        assert_eq!(line.character_id, role.character.id);
        assert_eq!(line.character_name, role.character.name);
        assert_eq!(line.has_saved_audio, input.step_ids[index + 2] == child);
    }
    let sessions = shared.lock().unwrap();
    assert_eq!(sessions.store.load_job(&parent).unwrap().unwrap(), before);
    assert_eq!(
        sessions.store.load(&before.session_id).unwrap().unwrap(),
        project_before
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_reports_frozen_connection_without_engine_discovery() {
    let root = std::env::temp_dir().join(format!("reuse-source-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let database = root.join("db");
    let recipe: sungeki_project::BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    let mut args = sungeki_protocol::StartBuildVideoArgs {
        wait_ms: None,
        job_id: uuid::Uuid::new_v4().to_string(),
        recipe,
        project_path: root.join("source.vvm.json").to_string_lossy().into(),
        base_url: "http://127.0.0.1:55021".into(),
        engine_version: "recorded".into(),
        managed_profiles: Default::default(),
        reuse_from: None,
    };
    let voice = &args.recipe.roles[0].character.voice;
    let sungeki_project::Voice::Voicevox {
        engine_id,
        core_version,
        ..
    } = voice
    else {
        panic!()
    };
    let connection = SpeechConnection {
        endpoint: args.base_url.clone(),
        identity: sungeki_speech::VoicevoxIdentity {
            engine_id: engine_id.clone(),
            core_version: core_version.clone(),
            engine_version: args.engine_version.clone(),
        },
    };
    let mut sessions = Sessions::open(&database).unwrap();
    create_configured(
        &mut sessions,
        &args.job_id,
        args.recipe,
        std::path::Path::new(&args.project_path),
        Some(connection),
        Default::default(),
    )
    .unwrap();
    drop(sessions);
    let shared = Arc::new(Mutex::new(Sessions::open(&database).unwrap()));
    let SessionResponse::BuildReuseSource { source } = execute(
        &shared,
        BuildVideoRequest::ReadReuseSource {
            job_id: std::mem::take(&mut args.job_id),
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(source.base_url, format!("{}/", args.base_url));
    assert_eq!(source.engine_version, args.engine_version);
    assert!(source.lines.iter().all(|line| !line.has_saved_audio));
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
