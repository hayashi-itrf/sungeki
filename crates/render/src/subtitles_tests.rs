use super::*;
use sungeki_project::{Clip, TimeUs};
fn fixture() -> (Project, Clip) {
    let mut p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let clip = p
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Speech { .. }))
        .unwrap()
        .clone();
    p.clips.clear();
    p.scenes.clear();
    p.groups.clear();
    for track in &mut p.tracks {
        track.visible = true;
    }
    (p, clip)
}
#[test]
fn captions_preserve_timeline_order_text_and_submillisecond_bounds() {
    let (mut p, mut clip) = fixture();
    clip.scene_id = None;
    clip.start_us = TimeUs(3_600_123_001);
    clip.duration_us = TimeUs(999);
    if let ClipContent::Speech {
        subtitle_text,
        spoken_text,
        audio,
        ..
    } = &mut clip.content
    {
        *subtitle_text = "Codex <設定> & 表示\n\n次の行".into();
        *spoken_text = Some("コーデックス".into());
        *audio = None;
    }
    let mut early = clip.clone();
    early.id = uuid::Uuid::new_v4().to_string();
    early.start_us = TimeUs(0);
    early.duration_us = TimeUs(1);
    p.clips = vec![clip, early];
    let srt = subtitle_document(&p, SubtitleFormat::Srt, false).unwrap();
    assert!(srt.starts_with("1\n00:00:00,000 --> 00:00:00,001\n"));
    assert!(srt.contains("2\n01:00:00,123 --> 01:00:00,124\n"));
    assert!(srt.contains("Codex &lt;設定&gt; &amp; 表示\n\u{a0}\n次の行"));
    assert!(!srt.contains("コーデックス"));
    let vtt = subtitle_document(&p, SubtitleFormat::WebVtt, false).unwrap();
    assert!(vtt.starts_with("WEBVTT\n\n1\n00:00:00.000 --> 00:00:00.001\n"));
}
#[test]
fn hidden_tracks_are_omitted_but_muting_does_not_hide_captions() {
    let (mut p, mut clip) = fixture();
    clip.scene_id = None;
    let track_id = clip.track_id.clone();
    p.clips.push(clip);
    let track = p.tracks.iter_mut().find(|t| t.id == track_id).unwrap();
    track.muted = true;
    assert!(
        !subtitle_document(&p, SubtitleFormat::Srt, false)
            .unwrap()
            .is_empty()
    );
    p.tracks
        .iter_mut()
        .find(|t| t.id == track_id)
        .unwrap()
        .visible = false;
    assert_eq!(
        subtitle_document(&p, SubtitleFormat::WebVtt, false).unwrap(),
        "WEBVTT\n\n"
    );
}
#[test]
fn text_cards_are_opt_in_and_invalid_timeline_is_rejected() {
    let (mut p, mut clip) = fixture();
    clip.scene_id = None;
    clip.track_id = p
        .tracks
        .iter()
        .find(|t| t.kind == sungeki_project::TrackKind::Visual)
        .unwrap()
        .id
        .clone();
    clip.content = ClipContent::Text {
        outline: None,
        layout: None,
        text: "題名".into(),
        font_size: 32.,
        color: "#ffffff".into(),
    };
    p.clips.push(clip);
    assert!(
        subtitle_document(&p, SubtitleFormat::Srt, false)
            .unwrap()
            .is_empty()
    );
    assert!(
        subtitle_document(&p, SubtitleFormat::Srt, true)
            .unwrap()
            .contains("題名")
    );
    p.clips[0].duration_us = TimeUs(0);
    assert!(subtitle_document(&p, SubtitleFormat::Srt, true).is_err());
}
