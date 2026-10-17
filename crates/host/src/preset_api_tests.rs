use super::*;
fn fixture_preset() -> sungeki_project::Preset {
    let recipe: sungeki_project::BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    sungeki_project::Preset {
        schema_version: 1,
        name: "会話の声".into(),
        description: String::new(),
        tags: vec![],
        content: PresetContent::Voice {
            voice: recipe.roles[0].character.voice.clone(),
            credit: "VOICEVOX:ずんだもん".into(),
        },
    }
}
fn call(
    shared: &Arc<Mutex<Sessions>>,
    request: PresetRequest,
) -> Result<SessionResponse, ApiError> {
    Sessions::execute(shared, SessionRequest::Presets { request })
}
#[test]
fn shared_host_persists_validates_and_rejects_stale_writes() {
    let fixture = crate::speech_journal_tests::Fixture::new();
    let path = fixture.0.join("host.sqlite");
    let shared = Arc::new(Mutex::new(Sessions::open(&path).unwrap()));
    let id = uuid::Uuid::new_v4().to_string();
    let save = |revision, preset| PresetRequest::Save {
        id: id.clone(),
        expected_revision: revision,
        preset,
    };
    let SessionResponse::Preset { record: saved } =
        call(&shared, save(0, fixture_preset())).unwrap()
    else {
        panic!()
    };
    assert_eq!(saved.revision, 1);
    let mut invalid = fixture_preset();
    invalid.name.clear();
    assert!(
        matches!(call(&shared, save(1, invalid)), Err(e) if e.code == ErrorCode::ValidationFailed)
    );
    let mut changed = fixture_preset();
    changed.name = "変更した声".into();
    assert!(
        matches!(call(&shared, save(0, changed.clone())), Err(e) if e.code == ErrorCode::Conflict)
    );
    call(&shared, save(1, changed.clone())).unwrap();
    drop(shared);
    let shared = Arc::new(Mutex::new(Sessions::open(&path).unwrap()));
    let SessionResponse::Preset { record } =
        call(&shared, PresetRequest::Get { id: id.clone() }).unwrap()
    else {
        panic!()
    };
    assert_eq!(record.preset, Some(changed));
    assert_eq!(record.revision, 2);
    call(
        &shared,
        PresetRequest::Delete {
            id: id.clone(),
            expected_revision: 2,
        },
    )
    .unwrap();
    assert!(
        matches!(call(&shared, save(0, fixture_preset())), Err(e) if e.code == ErrorCode::Conflict)
    );
    let SessionResponse::Presets {
        items,
        next_after_id,
    } = call(
        &shared,
        PresetRequest::List {
            after_id: None,
            limit: 20,
        },
    )
    .unwrap()
    else {
        panic!()
    };
    assert!(items.is_empty());
    assert!(next_after_id.is_none());
}

#[test]
fn pages_have_stable_cursors_and_no_large_payloads() {
    let fixture = crate::speech_journal_tests::Fixture::new();
    let shared = Arc::new(Mutex::new(
        Sessions::open(&fixture.0.join("host.sqlite")).unwrap(),
    ));
    for n in 1..=3 {
        call(
            &shared,
            PresetRequest::Save {
                id: uuid::Uuid::from_u128(n).to_string(),
                expected_revision: 0,
                preset: fixture_preset(),
            },
        )
        .unwrap();
    }
    let SessionResponse::Presets {
        items,
        next_after_id,
    } = call(
        &shared,
        PresetRequest::List {
            after_id: None,
            limit: 2,
        },
    )
    .unwrap()
    else {
        panic!()
    };
    assert_eq!(items.len(), 2);
    assert_eq!(next_after_id, Some(uuid::Uuid::from_u128(2).to_string()));
    let SessionResponse::Presets {
        items,
        next_after_id,
    } = call(
        &shared,
        PresetRequest::List {
            after_id: next_after_id,
            limit: 2,
        },
    )
    .unwrap()
    else {
        panic!()
    };
    assert_eq!(items[0].id, uuid::Uuid::from_u128(3).to_string());
    assert!(next_after_id.is_none());
}
