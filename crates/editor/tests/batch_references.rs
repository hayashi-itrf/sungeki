use std::collections::BTreeMap;
use sungeki_editor::*;
use sungeki_project::*;
fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn fixture() -> Project {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap()
}
fn setup() -> (Project, Vec<Command>, BTreeMap<String, String>) {
    let f = fixture();
    let p = Project::empty(id(100), "参照計画".into());
    let mut track = f.tracks[0].clone();
    track.id = "$track".into();
    track.name = "$person".into();
    let mut audio_track = f.tracks[1].clone();
    audio_track.id = "$audio_track".into();
    let mut text_track = f.tracks[0].clone();
    text_track.id = id(400);
    let mut character = f.characters[0].clone();
    character.id = "$person".into();
    character.name = "$track".into();
    let mut psd = f.assets[0].clone();
    psd.id = "$psd".into();
    let mut audio = f.assets[1].clone();
    audio.id = "$audio".into();
    let mut scene = f.scenes[0].clone();
    scene.id = "$scene".into();
    let mut standing = f.clips[0].clone();
    standing.id = "$standing".into();
    standing.track_id = "$track".into();
    standing.scene_id = Some("$scene".into());
    if let ClipContent::Psd {
        asset_id, speech, ..
    } = &mut standing.content
    {
        *asset_id = "$psd".into();
        speech.as_mut().unwrap().character_id = "$person".into();
    }
    let mut speech = f.clips[1].clone();
    speech.id = "$speech".into();
    speech.track_id = "$audio_track".into();
    speech.scene_id = Some("$scene".into());
    if let ClipContent::Speech {
        character_id,
        subtitle_text,
        spoken_text,
        audio,
        ..
    } = &mut speech.content
    {
        *character_id = "$person".into();
        *subtitle_text = "$track".into();
        *spoken_text = Some("$person".into());
        audio.as_mut().unwrap().asset_id = "$audio".into();
    }
    let mut text = speech.clone();
    text.id = "$text".into();
    text.track_id = id(400);
    text.content = ClipContent::Text {
        text: "$person".into(),
        font_size: 24.,
        color: "#ffffff".into(),
        layout: None,
        outline: None,
    };
    let group = Group {
        id: "$group".into(),
        name: "$scene".into(),
        clip_ids: vec!["$standing".into(), "$speech".into()],
    };
    let commands = vec![
        Command::AddTrack { track },
        Command::AddTrack { track: audio_track },
        Command::AddTrack { track: text_track },
        Command::SetCharacter { character },
        Command::RegisterAsset { asset: psd },
        Command::RegisterAsset { asset: audio },
        Command::AddScene { scene },
        Command::AddClip { clip: standing },
        Command::AddClip { clip: speech },
        Command::AddClip { clip: text },
        Command::AddGroup { group },
    ];
    let ids = [
        "track",
        "audio_track",
        "person",
        "psd",
        "audio",
        "scene",
        "standing",
        "speech",
        "text",
        "group",
    ]
    .iter()
    .enumerate()
    .map(|(i, n)| (n.to_string(), id(200 + i as u32)))
    .collect();
    (p, commands, ids)
}
#[test]
fn typed_creation_references_preserve_literal_text_and_undo_once() {
    let (p, commands, ids) = setup();
    let input = commands.clone();
    let plan = plan_batch_references(&p, &commands, &ids).unwrap();
    assert_eq!(commands, input);
    assert_eq!(plan.created_ids, ids);
    let result = apply_batch(&p, &plan.commands).unwrap();
    assert_eq!(result.tracks[0].name, "$person");
    assert_eq!(result.characters[0].name, "$track");
    assert_eq!(result.clips[0].scene_id.as_ref(), Some(&ids["scene"]));
    assert!(
        matches!(&result.clips[1].content,ClipContent::Speech{character_id,subtitle_text,spoken_text,audio:Some(a),..} if character_id==&ids["person"] && subtitle_text=="$track" && spoken_text.as_deref()==Some("$person") && a.asset_id==ids["audio"])
    );
    assert!(matches!(&result.clips[2].content,ClipContent::Text{text,..} if text=="$person"));
    assert_eq!(
        result.groups[0].clip_ids,
        vec![ids["standing"].clone(), ids["speech"].clone()]
    );
    let mut session = EditSession::new(p.clone()).unwrap();
    let request = EditRequest {
        operation_id: id(500),
        expected_revision: 0,
        action: EditAction::Apply {
            commands: plan.commands,
        },
    };
    let receipt = session.execute(request.clone()).unwrap();
    assert_eq!(session.execute(request).unwrap(), receipt);
    session
        .execute(EditRequest {
            operation_id: id(501),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: id(500),
            },
        })
        .unwrap();
    assert_eq!(session.history().project(), &p);
}
#[test]
fn invalid_allocations_duplicate_declarations_and_forward_references_fail_without_changes() {
    let (p, commands, ids) = setup();
    let original = p.clone();
    let mut missing = ids.clone();
    missing.remove("person");
    assert!(plan_batch_references(&p, &commands, &missing).is_err());
    let mut extra = ids.clone();
    extra.insert("unused".into(), id(999));
    assert!(plan_batch_references(&p, &commands, &extra).is_err());
    let mut collision = ids.clone();
    collision.insert("person".into(), p.id.clone());
    assert!(plan_batch_references(&p, &commands, &collision).is_err());
    collision.insert("person".into(), ids["track"].clone());
    assert!(plan_batch_references(&p, &commands, &collision).is_err());
    let mut forward = commands.clone();
    forward.swap(0, 7);
    assert!(plan_batch_references(&p, &forward, &ids).is_err());
    let mut duplicate = commands.clone();
    duplicate.push(commands[0].clone());
    assert!(plan_batch_references(&p, &duplicate, &ids).is_err());
    let mut invalid = ids.clone();
    invalid.insert("person".into(), "not-a-uuid".into());
    assert!(plan_batch_references(&p, &commands, &invalid).is_err());
    assert_eq!(p, original);
}

#[test]
fn public_edit_preview_allocations_recovery_and_followup_targets_are_stable() {
    let (p, mut commands, _) = setup();
    commands.push(Command::SetText {
        clip_id: "$text".into(),
        text: "文字列 $person は保持".into(),
    });
    let original = commands.clone();
    let request = EditRequest {
        operation_id: id(800),
        expected_revision: 0,
        action: EditAction::Apply { commands },
    };
    let mut preview = EditSession::new(p.clone()).unwrap();
    let mut preview_request = request.clone();
    preview_request.operation_id = id(801);
    let preview_receipt = preview.execute(preview_request).unwrap();
    let mut actual = EditSession::new(p.clone()).unwrap();
    let receipt = actual.execute(request.clone()).unwrap();
    assert_eq!(receipt.created_ids.as_ref().unwrap().len(), 10);
    assert_eq!(receipt.created_ids, preview_receipt.created_ids);
    assert_eq!(actual.history().project(), preview.history().project());
    assert_eq!(
        actual.recorded_request(&id(800)).unwrap().action,
        EditAction::Apply { commands: original }
    );
    assert!(
        matches!(&actual.history().project().clips[2].content,ClipContent::Text{text,..} if text=="文字列 $person は保持")
    );
    let mut restored = EditSession::restore(&actual.snapshot().unwrap()).unwrap();
    assert_eq!(restored.execute(request.clone()).unwrap(), receipt);
    restored
        .execute(EditRequest {
            operation_id: id(802),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: id(800),
            },
        })
        .unwrap();
    assert_eq!(restored.history().project(), &p);
    assert_eq!(restored.execute(request).unwrap(), receipt);
    assert_eq!(restored.history().project(), &p);
}

#[test]
fn duplicate_scene_resolves_sources_and_rejects_alias_key_collisions() {
    let (p, mut commands, ids) = setup();
    let mapping: BTreeMap<_, _> = ["standing", "speech", "text"]
        .iter()
        .enumerate()
        .map(|(i, name)| (format!("${name}"), id(900 + i as u32)))
        .collect();
    commands.push(Command::DuplicateScene {
        scene_id: "$scene".into(),
        new_scene_id: id(910),
        start_us: TimeUs(2000000),
        clip_ids: mapping.clone(),
        group_ids: BTreeMap::from([("$group".into(), id(911))]),
    });
    let plan = plan_batch_references(&p, &commands, &ids).unwrap();
    let result = apply_batch(&p, &plan.commands).unwrap();
    assert_eq!(result.scenes.len(), 2);
    assert_eq!(result.clips.len(), 6);
    assert_eq!(result.groups.len(), 2);
    for clip in &result.clips[3..] {
        assert_eq!(clip.scene_id.as_deref(), Some(id(910).as_str()));
        assert_eq!(clip.start_us, TimeUs(2000000));
    }
    assert_eq!(result.groups[1].clip_ids, vec![id(900), id(901)]);
    let mut invalid = commands.clone();
    if let Command::DuplicateScene { clip_ids, .. } = invalid.last_mut().unwrap() {
        clip_ids.insert(ids["standing"].clone(), id(920));
    }
    assert!(plan_batch_references(&p, &invalid, &ids).is_err());
    // A group copy has the same typed source-key contract.
    commands.pop();
    commands.push(Command::ResizeScene {
        scene_id: "$scene".into(),
        duration_us: TimeUs(4_000_000),
    });
    commands.push(Command::DuplicateGroup {
        group_id: "$group".into(),
        new_group_id: id(930),
        start_us: TimeUs(2_000_000),
        clip_ids: BTreeMap::from([("$standing".into(), id(931)), ("$speech".into(), id(932))]),
    });
    let plan = plan_batch_references(&p, &commands, &ids).unwrap();
    let result = apply_batch(&p, &plan.commands).unwrap();
    assert_eq!(result.groups[1].clip_ids, vec![id(931), id(932)]);
    assert_eq!(result.clips.len(), 5);
}
