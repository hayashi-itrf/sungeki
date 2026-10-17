use crate::{
    build_record::{
        self as build, Reservation,
        profiles::{Bindings, FrozenProfile, Kind},
    },
    speech_journal::{self, Input},
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use sungeki_speech::irodori::{Catalog, IrodoriJobInput};

#[test]
fn managed_parent_checks_profile_reservation_revision_and_cancellation() {
    let (root, mut sessions, fixture) = super::super::reservation_tests::setup(false);
    let mut recipe = build::decode(&fixture).unwrap().0.recipe;
    let identity: sungeki_project::IrodoriIdentity = serde_json::from_value(json!({
        "checkpoint":"model", "model_manifest_sha256":"a".repeat(64),
        "codec_manifest_sha256":"b".repeat(64), "engine_manifest_sha256":"c".repeat(64),
        "voice_manifest_sha256":"d".repeat(64), "sample_rate":24000,"channels":1
    }))
    .unwrap();
    recipe.roles[0].character.voice = sungeki_project::Voice::Irodori {
        identity: identity.clone(),
        settings: serde_json::from_value(json!({"voice_id":"none","caption":null,
            "cfg_scale":3.0,"num_steps":40,"seed":42}))
        .unwrap(),
    };
    for role in &mut recipe.roles {
        role.preset_id = "image-custom-v1".into();
    }
    let bindings = Bindings::from([(
        recipe.roles[0].character.id.clone(),
        FrozenProfile {
            kind: Kind::Irodori,
            profile_id: "local".into(),
            configuration_sha256: "e".repeat(64),
        },
    )]);
    let row = build::create_configured(
        &mut sessions,
        &uuid::Uuid::new_v4().to_string(),
        recipe,
        &root.join("managed-parent.vvm.json"),
        None,
        bindings,
    )
    .unwrap();
    let Reservation::New(prepare) = build::reserve(&mut sessions, &row).unwrap() else {
        panic!()
    };
    let shared = Arc::new(Mutex::new(sessions));
    build::prepare(&shared, &prepare).unwrap();
    let mut sessions = shared.lock().unwrap();
    let row = sessions.store.load_job(&row.id).unwrap().unwrap();
    let Reservation::New(compose) = build::reserve(&mut sessions, &row).unwrap() else {
        panic!()
    };
    build::compose(&mut sessions, &compose).unwrap();
    let row = sessions.store.load_job(&row.id).unwrap().unwrap();
    let Reservation::New(parent) = build::reserve(&mut sessions, &row).unwrap() else {
        panic!()
    };
    let (plan, state) = build::decode(&parent).unwrap();
    let child_id = state.child_id.unwrap();
    let (_, current) = sessions.load(&parent.session_id).unwrap();
    let project = current.history().project();
    let catalog = Catalog::new(
        serde_json::from_value(json!({"checkpoint":"model","speaker_conditioning":true,
            "caption_conditioning":false,"sample_rate":24000,"channels":1}))
        .unwrap(),
        serde_json::from_value(
            json!([{"id":"none","name":"None","reference":"none","caption":null}]),
        )
        .unwrap(),
    )
    .unwrap();
    let captured = IrodoriJobInput::new(
        state.expected_revision,
        project,
        &plan.plan.script.speech_ids[0],
        &identity,
        &catalog,
    )
    .unwrap()
    .for_parent(&parent.id)
    .unwrap();
    let input = Input::Irodori(captured.clone().for_profile("local").unwrap());
    let before = sessions.store.load(&parent.session_id).unwrap().unwrap();
    assert!(
        input
            .validate_parent(&sessions, "wrong-child", &parent.session_id)
            .is_err()
    );
    assert!(
        input
            .validate_parent(&sessions, &child_id, "wrong-session")
            .is_err()
    );
    for profile in [None, Some("other")] {
        let bad = match profile {
            Some(id) => captured.clone().for_profile(id).unwrap(),
            None => captured.clone(),
        };
        assert!(
            speech_journal::create_input(
                &mut sessions,
                child_id.clone(),
                parent.session_id.clone(),
                &Input::Irodori(bad)
            )
            .is_err()
        );
        assert!(sessions.store.load_job(&child_id).unwrap().is_none());
    }
    let stale = IrodoriJobInput::new(
        state.expected_revision + 1,
        input.project(),
        input.clip_id(),
        &identity,
        &catalog,
    )
    .unwrap()
    .for_profile("local")
    .unwrap()
    .for_parent(&parent.id)
    .unwrap();
    assert!(
        Input::Irodori(stale)
            .validate_parent(&sessions, &child_id, &parent.session_id)
            .is_err()
    );
    let child = speech_journal::create_input(
        &mut sessions,
        child_id.clone(),
        parent.session_id.clone(),
        &input,
    )
    .unwrap();
    let reservation =
        crate::speech_dispatch::Reservation::acquire(&mut sessions, &child_id).unwrap();
    build::request_cancel(&mut sessions, &parent).unwrap();
    assert!(reservation.begin(&mut sessions).is_err());
    assert_eq!(sessions.store.load_job(&child_id).unwrap().unwrap(), child);
    assert_eq!(
        sessions.store.load(&parent.session_id).unwrap().unwrap(),
        before
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
