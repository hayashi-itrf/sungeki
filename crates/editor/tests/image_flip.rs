use sungeki_editor::*;
use sungeki_project::*;
#[test]
fn reflected_psd_retains_pose_and_audio_and_is_undoable() {
    let p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let clip = p
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Psd { .. }))
        .unwrap();
    let mut t = clip.transform.clone();
    t.flip_x = Some(true);
    let mut h = History::new(p.clone()).unwrap();
    h.apply(
        0,
        &[Command::SetTransform {
            clip_id: clip.id.clone(),
            transform: t,
        }],
    )
    .unwrap();
    let next = h.project().clone();
    assert_eq!(
        next.clips.iter().find(|c| c.id == clip.id).unwrap().content,
        clip.content
    );
    let roundtrip: Project = serde_json::from_str(&serde_json::to_string(&next).unwrap()).unwrap();
    assert_eq!(next, roundtrip);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &next);
    let speech = next
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Speech { .. }))
        .unwrap();
    let mut t = speech.transform.clone();
    t.flip_x = Some(true);
    assert!(
        h.apply(
            3,
            &[Command::SetTransform {
                clip_id: speech.id.clone(),
                transform: t
            }]
        )
        .is_err()
    );
    assert_eq!(h.project(), &next);
}
