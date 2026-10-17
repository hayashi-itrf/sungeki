use sungeki_editor::*;
use sungeki_project::*;
fn id(n: u128) -> String {
    uuid::Uuid::from_u128(n).to_string()
}

fn fixture(source: &str) -> (Project, Preset, CharacterPresetIds, PresetStandingPlacement) {
    let mut source: Project = serde_json::from_str(source).unwrap();
    let recipe: BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    let character = source.characters.first().cloned().unwrap_or_else(|| {
        let mut character = recipe.roles[0].character.clone();
        character.id = id(7000);
        character
    });
    let clip = &source.clips[0];
    let standing = match &clip.content {
        ClipContent::Psd {
            asset_id,
            pose,
            speech,
        } => BuildStanding::Psd {
            asset_id: asset_id.clone(),
            pose: pose.clone(),
            speech: speech.clone().unwrap(),
            flip_x: true,
        },
        ClipContent::Sprite { sprites, pose, .. } => BuildStanding::Sprite {
            sprites: sprites.clone(),
            pose: pose.clone(),
            flip_x: true,
        },
        ClipContent::Vrm { standing } => {
            let mut standing = standing.clone();
            standing.character_id = Some(character.id.clone());
            BuildStanding::Vrm {
                standing,
                flip_x: true,
            }
        }
        _ => panic!(),
    };
    for asset in &mut source.assets {
        asset.credit = "Test fixture".into();
    }
    let mut character = character;
    character.credit = "Test fixture".into();
    let ids = CharacterPresetIds {
        character_id: id(1000),
        asset_ids: source
            .assets
            .iter()
            .enumerate()
            .map(|(n, a)| (a.id.clone(), id(2000 + n as u128)))
            .collect(),
    };
    let mut project = Project::empty(id(9000), "適用先".into());
    project.tracks.push(Track {
        id: id(9001),
        name: "立ち絵".into(),
        visible: true,
        muted: false,
        kind: TrackKind::Visual,
    });
    let placement = PresetStandingPlacement {
        new_track_name: None,
        clip_id: id(9002),
        track_id: id(9001),
        scene_id: None,
        start_us: TimeUs(0),
        duration_us: TimeUs(1_000_000),
        transform: clip.transform.clone(),
    };
    let preset = Preset {
        schema_version: 1,
        name: "再利用".into(),
        description: String::new(),
        tags: vec![],
        content: PresetContent::Character {
            character,
            assets: source.assets,
            standing: Some(standing),
        },
    };
    (project, preset, ids, placement)
}

#[test]
fn all_standing_bindings_remap_and_whole_application_undoes_once() {
    for source in [
        include_str!("../../../tests/fixtures/psd-speech-project.json"),
        include_str!("../../../tests/fixtures/sprite-project.json"),
        include_str!("../../../tests/fixtures/vrm-clip-project.json"),
    ] {
        let (project, preset, ids, placement) = fixture(source);
        let commands = plan_character_preset(&project, &preset, &ids, Some(&placement)).unwrap();
        let mut session = EditSession::new(project.clone()).unwrap();
        let request = EditRequest {
            operation_id: id(8000),
            expected_revision: 0,
            action: EditAction::Apply { commands },
        };
        let receipt = session.execute(request.clone()).unwrap();
        assert_eq!(session.execute(request).unwrap(), receipt);
        let result = session.history().project();
        assert_eq!(result.characters[0].id, ids.character_id);
        assert_eq!(result.clips[0].transform.flip_x, Some(true));
        match &result.clips[0].content {
            ClipContent::Psd {
                asset_id, speech, ..
            } => {
                assert!(ids.asset_ids.values().any(|v| v == asset_id));
                assert_eq!(speech.as_ref().unwrap().character_id, ids.character_id);
            }
            ClipContent::Sprite {
                sprites,
                character_id,
                ..
            } => {
                assert_eq!(character_id.as_ref(), Some(&ids.character_id));
                assert!(
                    sprites
                        .asset_ids()
                        .iter()
                        .all(|id| ids.asset_ids.values().any(|v| v == *id))
                );
            }
            ClipContent::Vrm { standing } => {
                assert_eq!(standing.character_id.as_ref(), Some(&ids.character_id));
                assert!(
                    ids.asset_ids
                        .values()
                        .any(|v| v == &standing.model_asset_id)
                );
                assert!(
                    ids.asset_ids
                        .values()
                        .any(|v| v == &standing.motion_asset_id)
                );
            }
            _ => panic!(),
        }
        session
            .execute(EditRequest {
                operation_id: id(8001),
                expected_revision: 1,
                action: EditAction::Undo {
                    expected_operation_id: id(8000),
                },
            })
            .unwrap();
        assert_eq!(session.history().project(), &project);
    }
}

#[test]
fn collisions_missing_mappings_and_invalid_placement_leave_original_unchanged() {
    let (project, preset, mut ids, mut placement) = fixture(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ));
    let original = project.clone();
    ids.character_id = project.tracks[0].id.clone();
    assert!(plan_character_preset(&project, &preset, &ids, Some(&placement)).is_err());
    ids.character_id = id(1000);
    placement.track_id = id(99999);
    assert!(plan_character_preset(&project, &preset, &ids, Some(&placement)).is_err());
    placement.track_id = project.tracks[0].id.clone();
    ids.asset_ids.clear();
    assert!(plan_character_preset(&project, &preset, &ids, Some(&placement)).is_err());
    assert_eq!(project, original);
}

#[test]
fn image_and_subtitle_only_presets_preserve_their_settings() {
    let (project, mut preset, mut ids, placement) = fixture(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ));
    let recipe: BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    ids.asset_ids = recipe
        .assets
        .iter()
        .enumerate()
        .map(|(n, a)| (a.id.clone(), id(3000 + n as u128)))
        .collect();
    preset.content = PresetContent::Character {
        character: recipe.roles[0].character.clone(),
        assets: recipe.assets,
        standing: Some(recipe.roles[0].standing.clone()),
    };
    let commands = plan_character_preset(&project, &preset, &ids, Some(&placement)).unwrap();
    let result = apply_batch(&project, &commands).unwrap();
    assert!(
        matches!(&result.clips[0].content, ClipContent::Image { asset_id } if ids.asset_ids.values().any(|v| v == asset_id))
    );
    assert_eq!(
        result.characters[0].subtitle_style,
        recipe.roles[0].character.subtitle_style
    );
    let PresetContent::Character {
        assets, standing, ..
    } = &mut preset.content
    else {
        panic!()
    };
    assets.clear();
    *standing = None;
    ids.asset_ids.clear();
    let commands = plan_character_preset(&project, &preset, &ids, None).unwrap();
    let result = apply_batch(&project, &commands).unwrap();
    assert_eq!(result.characters.len(), 1);
    assert!(result.clips.is_empty());
    assert!(result.assets.is_empty());
}

#[test]
fn empty_project_creates_track_atomically_and_undo_removes_it() {
    let (mut project, preset, ids, mut placement) = fixture(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ));
    project.tracks.clear();
    placement.new_track_name = Some("立ち絵".into());
    let commands = plan_character_preset(&project, &preset, &ids, Some(&placement)).unwrap();
    let mut session = EditSession::new(project.clone()).unwrap();
    session
        .execute(EditRequest {
            operation_id: id(8100),
            expected_revision: 0,
            action: EditAction::Apply { commands },
        })
        .unwrap();
    assert_eq!(session.history().project().tracks.len(), 1);
    assert_eq!(
        session.history().project().clips[0].track_id,
        placement.track_id
    );
    session
        .execute(EditRequest {
            operation_id: id(8101),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: id(8100),
            },
        })
        .unwrap();
    assert_eq!(session.history().project(), &project);
    placement.track_id = ids.character_id.clone();
    assert!(plan_character_preset(&project, &preset, &ids, Some(&placement)).is_err());
}

#[test]
fn preset_placement_uses_track_and_scene_created_in_same_batch() {
    use std::collections::BTreeMap;
    let (mut project, preset, ids, mut placement) = fixture(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ));
    let mut track = project.tracks.remove(0);
    let track_id = track.id.clone();
    track.id = "$track".into();
    placement.track_id = "$track".into();
    placement.scene_id = Some("$scene".into());
    let commands = vec![
        Command::AddTrack { track },
        Command::AddScene {
            scene: Scene {
                id: "$scene".into(),
                name: "scene".into(),
                start_us: TimeUs(0),
                duration_us: TimeUs(1000000),
                fade: None,
            },
        },
        Command::ApplyCharacterPreset {
            preset_id: id(5000),
            preset_revision: 1,
            preset,
            ids,
            placement: Some(placement),
        },
    ];
    let plan = plan_batch_references(
        &project,
        &commands,
        &BTreeMap::from([
            ("track".into(), track_id.clone()),
            ("scene".into(), id(5001)),
        ]),
    )
    .unwrap();
    let result = apply_batch(&project, &plan.commands).unwrap();
    assert_eq!(result.clips[0].track_id, track_id);
    assert_eq!(result.clips[0].scene_id.as_deref(), Some(id(5001).as_str()));
}
