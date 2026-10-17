use sungeki_editor::*;
use sungeki_project::*;

#[path = "common/build_recipe.rs"]
mod setup;
use setup::*;

#[test]
fn recipe_speech_preserves_order_scene_text_and_replay_across_restore_and_undo() {
    let (p, mut recipe, ids) = fixture();
    for role in &mut recipe.roles {
        role.character.subtitle_style.outline = Some(SubtitleOutline {
            width: 2,
            color: "#000000".into(),
        });
        role.character.subtitle_style.layout = Some(SubtitleLayout {
            alignment: TextAlignment::Right,
            wrap: false,
        });
    }
    let commands = plan_build_script(&p, &recipe, &ids).unwrap();
    assert_eq!(plan_build_script(&p, &recipe, &ids).unwrap(), commands);
    let request = EditRequest {
        operation_id: id(100),
        expected_revision: 0,
        action: EditAction::Apply { commands },
    };
    let mut session = EditSession::new(p.clone()).unwrap();
    let receipt = session.execute(request.clone()).unwrap();
    let composed = session.history().project().clone();
    assert_eq!(composed.clips.len(), 4);
    assert_eq!(composed.scenes.len(), 2);
    assert_eq!(
        composed.characters,
        recipe
            .roles
            .iter()
            .map(|r| r.character.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(composed.assets, recipe.assets);
    let mut index = 0;
    for (scene_index, scene) in recipe.scenes.iter().enumerate() {
        for line in &scene.lines {
            let clip = &composed.clips[index];
            assert_eq!(clip.id, ids.speech_ids[index]);
            assert_eq!(clip.scene_id, Some(ids.scene_ids[scene_index].clone()));
            assert!(matches!(&clip.content, ClipContent::Speech {
                character_id, subtitle_text, spoken_text, audio: None, ..
            } if *character_id == recipe.roles.iter().find(|r| r.role == line.role).unwrap().character.id
                && *subtitle_text == line.subtitle_text && *spoken_text == line.spoken_text));
            index += 1;
        }
    }
    let mut restored = EditSession::restore(&session.snapshot().unwrap()).unwrap();
    assert_eq!(restored.execute(request.clone()).unwrap(), receipt);
    assert_eq!(restored.history().project(), &composed);
    restored
        .execute(EditRequest {
            operation_id: id(101),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: id(100),
            },
        })
        .unwrap();
    assert_eq!(restored.history().project(), &p);
    assert_eq!(restored.execute(request).unwrap(), receipt);
    assert_eq!(restored.history().project(), &p);
    restored
        .execute(EditRequest {
            operation_id: id(102),
            expected_revision: 2,
            action: EditAction::Redo {
                expected_operation_id: id(100),
            },
        })
        .unwrap();
    assert_eq!(restored.history().project(), &composed);
}

#[test]
fn refuses_existing_work_and_bad_fixed_ids_without_mutation() {
    let (p, recipe, ids) = fixture();
    let result = apply_batch(&p, &plan_build_script(&p, &recipe, &ids).unwrap()).unwrap();
    assert!(plan_build_script(&result, &recipe, &ids).is_err());
    for mode in 0..5 {
        let mut bad = ids.clone();
        match mode {
            0 => {
                bad.speech_ids.pop();
            }
            1 => {
                bad.scene_ids.pop();
            }
            2 => bad.speech_ids[1] = bad.speech_ids[0].clone(),
            3 => bad.track_id = recipe.assets[0].id.clone(),
            _ => bad.scene_ids[0] = "bad-id".into(),
        }
        assert!(plan_build_script(&p, &recipe, &bad).is_err());
    }
    assert!(p.clips.is_empty());
}

#[test]
fn templates_wrap_unspecified_subtitles_without_changing_recipe_or_voice() {
    for preset in [
        "dialogue-wide-v1",
        "narration-wide-v1",
        "narration-portrait-v1",
    ] {
        let (p, mut recipe, ids) = fixture();
        recipe.preset_id = preset.into();
        recipe.foreword = None;
        if preset == "narration-portrait-v1" {
            recipe.roles[0].character.subtitle_style.color = "#FFFFFF".into();
        }
        if preset != "dialogue-wide-v1" {
            recipe.roles.truncate(1);
            for scene in &mut recipe.scenes {
                for line in &mut scene.lines {
                    line.role = recipe.roles[0].role.clone();
                }
            }
        }
        let original = recipe.clone();
        let result = apply_batch(&p, &plan_build_script(&p, &recipe, &ids).unwrap()).unwrap();
        for (actual, role) in result.characters.iter().zip(&recipe.roles) {
            let mut expected = role.character.clone();
            expected.subtitle_style.layout = Some(SubtitleLayout {
                alignment: TextAlignment::Left,
                wrap: true,
            });
            expected.subtitle_style.outline = Some(SubtitleOutline {
                width: 3,
                color: if preset == "narration-portrait-v1" {
                    "#000000"
                } else {
                    "#ffffff"
                }
                .into(),
            });
            assert_eq!(*actual, expected);
        }
        assert_eq!(recipe, original);
    }
}
