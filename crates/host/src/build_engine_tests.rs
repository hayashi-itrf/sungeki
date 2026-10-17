use super::*;
use crate::build_record::SpeechConnection;
use sungeki_project::Voice;
#[test]
fn engine_binding_is_fixed_across_replay_and_recovery() {
    let (root, mut sessions, fixture) = super::reservation_tests::setup(false);
    let recipe = decode(&fixture).unwrap().0.recipe;
    let Voice::Voicevox {
        engine_id,
        core_version,
        ..
    } = &recipe.roles[0].character.voice
    else {
        panic!("VOICEVOX fixture required");
    };
    let connection = SpeechConnection {
        endpoint: "http://127.0.0.1:55021/".into(),
        identity: sungeki_speech::VoicevoxIdentity {
            engine_id: engine_id.clone(),
            core_version: core_version.clone(),
            engine_version: "1.0".into(),
        },
    };
    let id = uuid::Uuid::new_v4().to_string();
    let path = root.join("合成先固定.vvm.json");
    let row = crate::build_record::create_bound(
        &mut sessions,
        &id,
        recipe.clone(),
        &path,
        Some(connection.clone()),
    )
    .unwrap();
    let bound = decode(&row).unwrap().0.engine.unwrap();
    assert_eq!(bound.endpoint, "http://127.0.0.1:55021/");
    assert_eq!(
        crate::build_record::create_bound(
            &mut sessions,
            &id,
            recipe.clone(),
            &path,
            Some(bound.clone())
        )
        .unwrap(),
        row
    );
    assert!(crate::build_record::create(&mut sessions, &id, recipe.clone(), &path).is_err());
    for variant in 0..3 {
        let mut changed = bound.clone();
        match variant {
            0 => changed.endpoint = "http://127.0.0.1:55022".into(),
            1 => changed.identity.engine_version = "2.0".into(),
            _ => changed.identity.core_version = "different".into(),
        }
        assert!(
            crate::build_record::create_bound(
                &mut sessions,
                &id,
                recipe.clone(),
                &path,
                Some(changed)
            )
            .is_err()
        );
    }
    assert_eq!(sessions.store.load_job(&id).unwrap().unwrap(), row);
    drop(sessions);
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let restored =
        crate::build_record::create_bound(&mut sessions, &id, recipe, &path, Some(connection))
            .unwrap();
    assert_eq!(restored.input, row.input);
    assert_eq!(decode(&restored).unwrap().1.phase, JobPhase::Interrupted);
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn mismatched_role_and_invalid_endpoint_do_not_create_a_parent() {
    let (root, mut sessions, fixture) = super::reservation_tests::setup(false);
    let recipe = decode(&fixture).unwrap().0.recipe;
    let connection = SpeechConnection {
        endpoint: "https://example.invalid".into(),
        identity: sungeki_speech::VoicevoxIdentity {
            engine_id: "wrong".into(),
            core_version: "0.16.1".into(),
            engine_version: "1.0".into(),
        },
    };
    let id = uuid::Uuid::new_v4().to_string();
    assert!(
        crate::build_record::create_bound(
            &mut sessions,
            &id,
            recipe,
            &root.join("拒否.vvm.json"),
            Some(connection)
        )
        .is_err()
    );
    assert!(sessions.store.load_job(&id).unwrap().is_none());
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn runtime_cannot_override_saved_engine_before_acquiring_execution() {
    use std::sync::{Arc, Mutex, atomic::Ordering};
    let (root, mut sessions, fixture) = super::reservation_tests::setup(false);
    let recipe = decode(&fixture).unwrap().0.recipe;
    let Voice::Voicevox {
        engine_id,
        core_version,
        ..
    } = &recipe.roles[0].character.voice
    else {
        panic!("VOICEVOX fixture required");
    };
    let connection = SpeechConnection {
        endpoint: "http://127.0.0.1:55021/".into(),
        identity: sungeki_speech::VoicevoxIdentity {
            engine_id: engine_id.clone(),
            core_version: core_version.clone(),
            engine_version: "1.0".into(),
        },
    };
    let parent = crate::build_record::create_bound(
        &mut sessions,
        &uuid::Uuid::new_v4().to_string(),
        recipe,
        &root.join("接続変更拒否.vvm.json"),
        Some(connection.clone()),
    )
    .unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    let mut other = connection;
    other.endpoint = "http://127.0.0.1:55022/".into();
    assert!(crate::build_record::launch(&shared, &parent.id, Some(other)).is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .build_runtime
            .active
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        parent
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
