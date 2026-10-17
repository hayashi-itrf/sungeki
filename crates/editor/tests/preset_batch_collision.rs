use sungeki_editor::*;
use sungeki_project::*;

fn id(n: u128) -> String {
    uuid::Uuid::from_u128(n).to_string()
}

#[test]
fn preset_collision_is_checked_after_the_complete_batch() {
    let recipe: BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    let preset = Preset {
        schema_version: 1,
        name: "立ち絵".into(),
        description: String::new(),
        tags: vec![],
        content: PresetContent::Character {
            character: recipe.roles[0].character.clone(),
            assets: recipe.assets.clone(),
            standing: Some(recipe.roles[0].standing.clone()),
        },
    };
    let mut p = Project::empty(id(1), "プリセットの一括再配置".into());
    p.tracks.push(Track {
        id: id(2),
        name: "映像".into(),
        kind: TrackKind::Visual,
        visible: true,
        muted: false,
    });
    let occupied: Clip = serde_json::from_value(serde_json::json!({
        "id":id(3), "track_id":id(2), "scene_id":null, "start_us":0,
        "duration_us":1_000_000,
        "transform":{"x":0,"y":0,"width":100,"height":100,"opacity":1},
        "content":{"type":"shape","shape":"rectangle","fill":"#00ff00"}
    }))
    .unwrap();
    p.clips.push(occupied.clone());
    let ids = CharacterPresetIds {
        character_id: id(4),
        asset_ids: recipe
            .assets
            .iter()
            .enumerate()
            .map(|(n, a)| (a.id.clone(), id(100 + n as u128)))
            .collect(),
    };
    let placement = PresetStandingPlacement {
        new_track_name: None,
        clip_id: id(5),
        track_id: id(2),
        scene_id: None,
        start_us: TimeUs(0),
        duration_us: TimeUs(1_000_000),
        transform: occupied.transform,
    };
    // A standalone plan still rejects the overlap.
    assert!(plan_character_preset(&p, &preset, &ids, Some(&placement)).is_err());
    let command = Command::ApplyCharacterPreset {
        preset_id: id(6),
        preset_revision: 1,
        preset,
        ids,
        placement: Some(placement),
    };
    let mut h = History::new(p.clone()).unwrap();
    assert!(h.apply(0, &[command.clone()]).is_err());
    assert_eq!(h.project(), &p);
    assert_eq!(h.revision(), 0);
    assert_eq!(h.undo_len(), 0);
    let commands = [
        command,
        Command::MoveClip {
            clip_id: id(5),
            start_us: TimeUs(1_000_000),
        },
    ];
    let preview = apply_batch(&p, &commands).unwrap();
    h.apply(0, &commands).unwrap();
    assert_eq!(h.project(), &preview);
    assert_eq!(
        preview
            .clips
            .iter()
            .find(|c| c.id == id(5))
            .unwrap()
            .start_us,
        TimeUs(1_000_000)
    );
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &preview);
}
