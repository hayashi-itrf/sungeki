use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/speech.rs"]
mod setup;
#[test]
fn fade_is_atomic_undoable_and_does_not_change_speech_audio() {
    let p = apply_batch(
        &Project::empty(setup::id(1), "fade".into()),
        &setup::commands(),
    )
    .unwrap();
    let clip = &p.clips[0];
    let mut transform = clip.transform.clone();
    transform.fade = Some(VisualFade {
        in_us: TimeUs(100_000),
        out_us: TimeUs(100_000),
    });
    let command = Command::SetTransform {
        clip_id: clip.id.clone(),
        transform,
    };
    let mut history = History::new(p.clone()).unwrap();
    history.apply(0, &[command]).unwrap();
    let next = history.project().clone();
    assert_eq!(next.clips[0].content, clip.content);
    assert!(
        history
            .apply(
                1,
                &[Command::ResizeClip {
                    clip_id: clip.id.clone(),
                    duration_us: TimeUs(100_000)
                }]
            )
            .is_err()
    );
    assert_eq!(history.project(), &next);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &p);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &next);
}
