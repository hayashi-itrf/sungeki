use sungeki_editor::{Command, History, apply_batch};
use sungeki_project::*;
fn fixture() -> Project {
    serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json")).unwrap()
}
#[test]
fn video_registration_undo_redo_and_invalid_batch_are_shared() {
    let p = fixture();
    let empty = Project::empty(p.id.clone(), p.title.clone());
    let asset = p.assets[0].clone();
    let mut h = History::new(empty.clone()).unwrap();
    h.apply(
        0,
        &[Command::RegisterAsset {
            asset: asset.clone(),
        }],
    )
    .unwrap();
    let registered = h.project().clone();
    validate(&registered).unwrap();
    let loaded: Project =
        serde_json::from_slice(&serde_json::to_vec(&registered).unwrap()).unwrap();
    assert_eq!(loaded, registered);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &empty);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &registered);
    let mut bad = asset.clone();
    bad.id = uuid::Uuid::new_v4().to_string();
    let AssetProbe::Video(probe) = &mut bad.probe else {
        panic!()
    };
    probe.metadata.video.clock.time_base.denominator = 0;
    assert!(
        apply_batch(
            &empty,
            &[
                Command::RegisterAsset { asset },
                Command::RegisterAsset { asset: bad }
            ]
        )
        .is_err()
    );
    assert!(empty.assets.is_empty());
}
