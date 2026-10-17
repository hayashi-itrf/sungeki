use super::*;
use serde_json::json;
fn recipe() -> BuildRecipe {
    let mut recipe: BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe.roles[0].character.voice = serde_json::from_value(json!({
        "type":"irodori", "settings":{"voice_id":"none","caption":null,"cfg_scale":3.0,"num_steps":40,"seed":42},
        "identity":{"checkpoint":"test","model_manifest_sha256":"a".repeat(64),"codec_manifest_sha256":"b".repeat(64),"engine_manifest_sha256":"c".repeat(64),"voice_manifest_sha256":"d".repeat(64),"sample_rate":44100,"channels":1}
    })).unwrap();
    recipe
}
#[test]
fn configuration_digest_covers_engine_profile_paths_metadata_and_settings() {
    let value = json!({"binding":{"path":"/original"},"catalog":{"speaker":"narrator"},"seed":42});
    let original = freeze(Kind::Irodori, "local", &value).unwrap();
    let reordered =
        json!({"seed":42,"catalog":{"speaker":"narrator"},"binding":{"path":"/original"}});
    assert_eq!(
        original,
        freeze(Kind::Irodori, "local", &reordered).unwrap()
    );
    assert_ne!(original, freeze(Kind::Qwen, "local", &value).unwrap());
    assert_ne!(original, freeze(Kind::Irodori, "other", &value).unwrap());
    for changed in [
        json!({"binding":{"path":"/changed"},"catalog":{"speaker":"narrator"},"seed":42}),
        json!({"binding":{"path":"/original"},"catalog":{"speaker":"other"},"seed":42}),
        json!({"binding":{"path":"/original"},"catalog":{"speaker":"narrator"},"seed":43}),
    ] {
        assert_ne!(original, freeze(Kind::Irodori, "local", &changed).unwrap());
    }
}
#[test]
fn exact_managed_character_coverage_is_required() {
    let recipe = recipe();
    let id = recipe.roles[0].character.id.clone();
    let profile = freeze(Kind::Irodori, "local", &json!({})).unwrap();
    let mut bindings = Bindings::from([(id.clone(), profile.clone())]);
    validate(&bindings, &recipe).unwrap();
    assert!(validate(&Bindings::new(), &recipe).is_err());
    bindings.insert("unknown".into(), profile.clone());
    assert!(validate(&bindings, &recipe).is_err());
    bindings.remove("unknown");
    for bad in [
        FrozenProfile {
            kind: Kind::Sbv2,
            ..profile.clone()
        },
        FrozenProfile {
            profile_id: "../other".into(),
            ..profile.clone()
        },
        FrozenProfile {
            configuration_sha256: "A".repeat(64),
            ..profile.clone()
        },
    ] {
        bindings.insert(id.clone(), bad);
        assert!(validate(&bindings, &recipe).is_err());
    }
}

#[test]
fn frozen_profile_survives_recovery_and_replay_rejects_changed_settings() {
    let root = std::env::temp_dir().join(format!("next-build-bindings-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let db = root.join("db");
    let recipe = recipe();
    let bindings = Bindings::from([(
        recipe.roles[0].character.id.clone(),
        freeze(Kind::Irodori, "local", &json!({"seed":42})).unwrap(),
    )]);
    let mut sessions = crate::sessions::Sessions::open(&db).unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let path = root.join("managed.vvm.json");
    let row = crate::build_record::create_configured(
        &mut sessions,
        &id,
        recipe.clone(),
        &path,
        None,
        bindings.clone(),
    )
    .unwrap();
    assert_eq!(
        crate::build_record::decode(&row)
            .unwrap()
            .0
            .managed_profiles,
        bindings
    );
    assert_eq!(
        crate::build_record::create_configured(
            &mut sessions,
            &id,
            recipe.clone(),
            &path,
            None,
            bindings.clone()
        )
        .unwrap(),
        row
    );
    drop(sessions);
    let mut sessions = crate::sessions::Sessions::open(&db).unwrap();
    let saved = sessions.store.load_job(&id).unwrap().unwrap();
    assert_eq!(
        crate::build_record::decode(&saved)
            .unwrap()
            .0
            .managed_profiles,
        bindings
    );
    let mut changed = bindings.clone();
    changed.values_mut().next().unwrap().configuration_sha256 = "e".repeat(64);
    assert!(
        crate::build_record::create_configured(&mut sessions, &id, recipe, &path, None, changed)
            .is_err()
    );
    assert_eq!(sessions.store.load_job(&id).unwrap().unwrap(), saved);
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}

/// Exercise the common parent capture against each engine's measured fixture.
pub(crate) fn check_prepared_child(
    profile: LoadedProfile,
    original: &crate::speech_journal::Input,
) {
    let frozen = profile.freeze().unwrap();
    let parent = uuid::Uuid::new_v4().to_string();
    let prepare = |fixed: &FrozenProfile, flag: bool| {
        profile.prepare_child(
            fixed,
            &parent,
            original.source_revision(),
            original.project(),
            original.clip_id(),
            &AtomicBool::new(flag),
        )
    };
    let input = prepare(&frozen, false).unwrap();
    assert_eq!(input.parent_job_id(), Some(parent.as_str()));
    assert_eq!(input.project(), original.project());
    assert_eq!(input.clip_id(), original.clip_id());
    assert_eq!(input.source_revision(), original.source_revision());
    let profile_id = match &input {
        crate::speech_journal::Input::Sbv2(i) => i.profile_id(),
        crate::speech_journal::Input::Qwen(i) => i.profile_id(),
        crate::speech_journal::Input::Irodori(i) => i.profile_id(),
        _ => panic!("managed capture fell back to VOICEVOX"),
    };
    assert_eq!(profile_id, Some(frozen.profile_id.as_str()));
    let bytes = serde_json::to_vec(&input).unwrap();
    let restored: crate::speech_journal::Input = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored.input_hash(), input.input_hash());
    assert_eq!(restored.parent_job_id(), Some(parent.as_str()));
    assert_ne!(input.input_hash(), original.input_hash());
    let mut changed = frozen.clone();
    changed.configuration_sha256 = "0".repeat(64);
    assert!(prepare(&changed, false).is_err());
    assert_eq!(
        prepare(&frozen, true).unwrap_err().code,
        sungeki_protocol::ErrorCode::Cancelled
    );
}
