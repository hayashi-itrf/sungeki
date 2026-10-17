use sungeki_editor::{Command, History, apply_batch};
use sungeki_project::*;
fn project() -> Project {
    serde_json::from_str(include_str!("../../../tests/fixtures/sprite-project.json")).unwrap()
}
#[test]
fn sprite_pose_edit_is_atomic_undoable_and_serializable() {
    let p = project();
    let ClipContent::Sprite {
        sprites,
        character_id,
        ..
    } = p.clips[0].content.clone()
    else {
        panic!()
    };
    let command = |pose: &str| Command::SetSprite {
        clip_id: p.clips[0].id.clone(),
        sprites: sprites.clone(),
        pose: Some(pose.into()),
        character_id: character_id.clone(),
    };
    let mut h = History::new(p.clone()).unwrap();
    assert!(h.apply(0, &[command("笑顔"), command("unknown")]).is_err());
    assert_eq!(h.project(), &p);
    h.apply(0, &[command("笑顔")]).unwrap();
    let edited = h.project().clone();
    assert_eq!(
        serde_json::from_slice::<Project>(&serde_json::to_vec(&edited).unwrap()).unwrap(),
        edited
    );
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &edited);
    for id in sprites.asset_ids() {
        assert!(
            apply_batch(
                &edited,
                &[Command::RemoveAsset {
                    asset_id: id.into()
                }]
            )
            .is_err()
        );
    }
    assert!(
        apply_batch(
            &edited,
            &[Command::RemoveCharacter {
                character_id: character_id.unwrap()
            }]
        )
        .is_err()
    );
}
#[test]
fn sprite_add_requires_all_dependencies_in_same_batch() {
    let mut p = project();
    let clip = p.clips.remove(0);
    let image = p.assets.pop().unwrap();
    assert!(apply_batch(&p, &[Command::AddClip { clip: clip.clone() }]).is_err());
    let changed = apply_batch(
        &p,
        &[
            Command::RegisterAsset { asset: image },
            Command::AddClip { clip },
        ],
    )
    .unwrap();
    validate(&changed).unwrap();
}
