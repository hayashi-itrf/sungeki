use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/speech.rs"]
mod setup;
use setup::*;
fn fixture() -> Project {
    let mut p = apply_batch(&Project::empty(id(1), "説明図".into()), &commands()).unwrap();
    p.tracks.push(Track {
        id: id(8),
        name: "説明".into(),
        kind: TrackKind::Visual,
        visible: true,
        muted: false,
    });
    let mut last = p.clips[0].clone();
    last.id = id(7);
    last.start_us = TimeUs(3_000_000);
    p.clips.push(last);
    let mut visual = p.clips[0].clone();
    visual.id = id(9);
    visual.track_id = id(8);
    visual.start_us = TimeUs(1_000_000);
    visual.duration_us = TimeUs(500_000);
    visual.content = ClipContent::Text {
        outline: None,
        layout: None,
        text: "説明図の見出し".into(),
        font_size: 32.0,
        color: "#ffffff".into(),
    };
    p.clips.push(visual);
    validate(&p).unwrap();
    p
}
fn fit(first: u32, last: u32) -> Command {
    Command::FitVisualToSpeech {
        clip_id: id(9),
        first_speech_id: id(first),
        last_speech_id: id(last),
    }
}
#[test]
fn spans_speech_and_gap_preserves_content_and_undo() {
    let p = fixture();
    let mut h = History::new(p.clone()).unwrap();
    h.apply(0, &[fit(4, 7)]).unwrap();
    let next = h.project().clone();
    assert_eq!(next.clips[2].start_us, TimeUs(0));
    assert_eq!(next.clips[2].duration_us, TimeUs(5_000_000));
    assert_eq!(next.clips[..2], p.clips[..2]);
    assert_eq!(next.clips[2].content, p.clips[2].content);
    assert_eq!(next.clips[2].transform, p.clips[2].transform);
    assert_eq!(next.clips[2].scene_id, p.clips[2].scene_id);
    assert_eq!(
        next,
        serde_json::from_str::<Project>(&serde_json::to_string(&next).unwrap()).unwrap()
    );
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &next);
    let single = apply_batch(&p, &[fit(7, 7)]).unwrap();
    assert_eq!(single.clips[2].start_us, TimeUs(3_000_000));
    assert_eq!(single.clips[2].duration_us, TimeUs(2_000_000));
}
#[test]
fn invalid_references_and_order_are_atomic() {
    let p = fixture();
    for command in [
        fit(7, 4),
        fit(99, 7),
        fit(4, 9),
        Command::FitVisualToSpeech {
            clip_id: id(4),
            first_speech_id: id(4),
            last_speech_id: id(7),
        },
    ] {
        let mut h = History::new(p.clone()).unwrap();
        assert!(
            h.apply(
                0,
                &[
                    Command::RenameProject {
                        title: "rollback".into()
                    },
                    command
                ]
            )
            .is_err()
        );
        assert_eq!(h.project(), &p);
    }
}
#[test]
fn scene_bounds_fades_and_nested_speeches_are_rejected() {
    let mut p = fixture();
    p.clips[1].scene_id = None;
    p.clips[1].start_us = TimeUs(6_000_000);
    assert!(apply_batch(&p, &[fit(4, 7)]).is_err());
    let mut p = fixture();
    p.clips[2].duration_us = TimeUs(4_000_000);
    p.clips[2].transform.fade = Some(VisualFade {
        in_us: TimeUs(2_000_000),
        out_us: TimeUs(1_000_000),
    });
    assert!(apply_batch(&p, &[fit(7, 7)]).is_err());
    let mut p = fixture();
    p.clips[1].start_us = TimeUs(500_000);
    p.clips[1].duration_us = TimeUs(500_000);
    assert!(apply_batch(&p, &[fit(4, 7)]).is_err());
}
