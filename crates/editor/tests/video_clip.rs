use sungeki_editor::{Command, History, apply_batch};
use sungeki_project::*;
#[test]
fn video_trim_is_atomic_and_survives_move_undo_and_redo() {
    let p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap();
    let mut h = History::new(p.clone()).unwrap();
    let id = p.clips[0].id.clone();
    let ClipContent::Video {
        asset_id,
        mut playback,
    } = p.clips[0].content.clone()
    else {
        panic!()
    };
    playback.source_start_us = TimeUs(800000);
    playback.source_duration_us = TimeUs(200000);
    let update = Command::SetVideo {
        clip_id: id.clone(),
        asset_id,
        playback,
    };
    assert!(h.apply(0, &[update.clone()]).is_err());
    assert_eq!(h.project(), &p);
    h.apply(
        0,
        &[
            update,
            Command::ResizeClip {
                clip_id: id.clone(),
                duration_us: TimeUs(200000),
            },
            Command::MoveClip {
                clip_id: id,
                start_us: TimeUs(3000000),
            },
        ],
    )
    .unwrap();
    let edited = h.project().clone();
    assert_eq!(edited.clips[0].start_us, TimeUs(3000000));
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &edited);
    let deleted = apply_batch(
        &edited,
        &[Command::RemoveAsset {
            asset_id: p.assets[0].id.clone(),
        }],
    );
    assert!(deleted.is_err());
}
