use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/speech.rs"]
mod setup;
use setup::*;
fn fixture() -> Project {
    let mut p = apply_batch(&Project::empty(id(1), "台本".into()), &commands()).unwrap();
    let mut track = p.tracks[0].clone();
    track.id = id(50);
    p.tracks.push(track);
    p
}
fn script() -> SpeechScript {
    SpeechScript {
        lines: vec![
            SpeechScriptLine {
                character_id: None,
                clip_id: id(10),
                subtitle_text: "一行目".into(),
                spoken_text: Some("いちぎょうめ".into()),
            },
            SpeechScriptLine {
                character_id: None,
                clip_id: id(11),
                subtitle_text: "　 ".into(),
                spoken_text: None,
            },
            SpeechScriptLine {
                character_id: None,
                clip_id: id(12),
                subtitle_text: "二行目".into(),
                spoken_text: None,
            },
        ],
        character_id: id(2),
        track_id: id(50),
        scene_id: Some(id(5)),
        start_us: TimeUs(0),
        provisional_duration_us: TimeUs(1_000_000),
        gap_us: TimeUs(350_000),
    }
}
#[test]
fn ordered_lines_skip_blanks_preserve_membership_and_undo_together() {
    let p = fixture();
    let mut h = History::new(p.clone()).unwrap();
    h.apply(0, &[Command::AppendSpeechScript { script: script() }])
        .unwrap();
    let result = h.project();
    assert_eq!(result.clips.len(), 3);
    assert_eq!(result.clips[0], p.clips[0]);
    assert_eq!(result.clips[2].start_us, TimeUs(1_350_000));
    assert_eq!(result.clips[2].scene_id, Some(id(5)));
    assert!(
        matches!(&result.clips[1].content,ClipContent::Speech{spoken_text:Some(text),audio:None,..} if text=="いちぎょうめ")
    );
    let restored: Project = serde_json::from_str(&serde_json::to_string(result).unwrap()).unwrap();
    assert_eq!(&restored, result);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
}
#[test]
fn invalid_batch_is_atomic_including_earlier_commands() {
    for mode in 0..7 {
        let p = fixture();
        let mut s = script();
        match mode {
            0 => s.lines[2].clip_id = id(10),
            1 => s.provisional_duration_us = TimeUs(4_000_000),
            2 => s.start_us = TimeUs(MAX_TIME_US),
            3 => s.character_id = id(99),
            4 => s.lines.iter_mut().for_each(|l| {
                l.subtitle_text.clear();
                l.spoken_text = None;
            }),
            5 => s.lines = vec![s.lines[0].clone(); 201],
            _ => s.track_id = id(99),
        }
        let mut h = History::new(p.clone()).unwrap();
        assert!(
            h.apply(
                0,
                &[
                    Command::RenameProject {
                        title: "混入不可".into()
                    },
                    Command::AppendSpeechScript { script: s }
                ]
            )
            .is_err()
        );
        assert_eq!(h.project(), &p);
    }
}

#[test]
fn unicode_whitespace_is_skipped_but_bom_is_content() {
    let mut s = script();
    s.scene_id = None;
    s.lines = [
        "\u{85}",
        "\u{a0}\u{1680}\u{2000}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}",
        "\u{feff}",
        "本文",
    ]
    .iter()
    .enumerate()
    .map(|(n, text)| SpeechScriptLine {
        character_id: None,
        clip_id: id(20 + n as u32),
        subtitle_text: (*text).into(),
        spoken_text: None,
    })
    .collect();
    let p = fixture();
    let result = apply_batch(&p, &[Command::AppendSpeechScript { script: s }]).unwrap();
    assert_eq!(result.clips.len(), 3);
    assert_eq!(result.clips[1].id, id(22));
    assert_eq!(result.clips[2].start_us, TimeUs(1_350_000));
}

#[test]
fn dialogue_preserves_line_speakers_and_rolls_back_unknown_speaker() {
    let mut p = fixture();
    let mut second = character();
    second.id = id(30);
    second.name = "二人目".into();
    p.characters.push(second);
    let mut s = script();
    s.lines[2].character_id = Some(id(30));
    let mut h = History::new(p.clone()).unwrap();
    h.apply(0, &[Command::AppendSpeechScript { script: s.clone() }])
        .unwrap();
    let result = h.project().clone();
    assert!(
        matches!(&result.clips[1].content, ClipContent::Speech {character_id, spoken_text: Some(text),..} if *character_id == id(2) && text == "いちぎょうめ")
    );
    assert!(
        matches!(&result.clips[2].content, ClipContent::Speech {character_id,..} if *character_id == id(30))
    );
    assert_eq!(result.clips[2].scene_id, Some(id(5)));
    assert_eq!(
        serde_json::from_str::<Project>(&serde_json::to_string(&result).unwrap()).unwrap(),
        result
    );
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &result);
    s.lines[2].character_id = Some(id(99));
    assert!(apply_batch(&p, &[Command::AppendSpeechScript { script: s }]).is_err());
}

#[test]
fn explicit_empty_spoken_text_is_rejected_instead_of_silently_skipped() {
    for spoken in ["", "　 ", "\0"] {
        let p = fixture();
        let mut s = script();
        s.lines[1].spoken_text = Some(spoken.into());
        let mut h = History::new(p.clone()).unwrap();
        assert!(
            h.apply(0, &[Command::AppendSpeechScript { script: s }])
                .is_err()
        );
        assert_eq!(h.project(), &p);
    }
}
