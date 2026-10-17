use sungeki_project::*;
use sungeki_review::*;
fn project() -> Project {
    let mut p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    p.clips
        .retain(|c| matches!(c.content, ClipContent::Speech { .. }));
    p.scenes.clear();
    validate(&p).unwrap();
    p
}
fn add(p: &mut Project, start: u64, duration: u64) {
    let mut c = p.clips[0].clone();
    c.id = format!("00000000-0000-4000-8000-{:012}", 100 + p.clips.len());
    c.start_us = TimeUs(start);
    c.duration_us = TimeUs(duration);
    p.clips.push(c);
}
#[test]
fn overlap_is_warning_with_target_and_exact_intersection() {
    let mut p = project();
    add(&mut p, 500_000, 1_000_000);
    let original = p.clone();
    let (check, issues) = review_audio_timing(&p);
    assert_eq!(check.status, CheckStatus::NotRun); // Loudness/originals remain unverified.
    assert_eq!(issues.len(), 1);
    let i = &issues[0];
    assert_eq!(i.code, "SPEECH_OVERLAP");
    assert_eq!(i.severity, Severity::Warning);
    assert!(i.message.contains("発話「speech」（開始 0:00.500）"));
    assert!(i.message.contains("発話「speech」（開始 0:00.000）"));
    assert!(!i.message.contains(&p.clips[0].id));
    assert_eq!(i.clip_id.as_ref(), Some(&p.clips[1].id));
    assert_eq!(
        (i.start_us, i.end_us),
        (Some(TimeUs(500_000)), Some(TimeUs(1_000_000)))
    );
    assert_eq!(p, original);
}

#[test]
fn overlap_excerpt_is_bounded_and_preserves_japanese_without_line_breaks() {
    let mut p = project();
    if let ClipContent::Speech { subtitle_text, .. } = &mut p.clips[0].content {
        *subtitle_text = format!("日本語\n{}", "長".repeat(80));
    }
    add(&mut p, 500_000, 1_000_000);
    let (_, issues) = review_audio_timing(&p);
    let message = &issues[0].message;
    assert!(message.contains(&format!("日本語 {}…", "長".repeat(36))));
    assert!(!message.contains('\n'));
    assert!(!message.contains(&"長".repeat(37)));
}
#[test]
fn trailing_silence_and_touching_boundaries_are_not_overlaps() {
    let mut p = project();
    p.clips[0].duration_us = TimeUs(2_000_000);
    add(&mut p, 1_000_000, 1_000_000);
    assert!(review_audio_timing(&p).1.is_empty());
}
#[test]
fn truncation_fails_but_muting_excludes_it_and_visibility_does_not() {
    let mut p = project();
    p.clips[0].duration_us = TimeUs(999_999);
    let (check, issues) = review_audio_timing(&p);
    assert_eq!(check.status, CheckStatus::Failed);
    assert_eq!(issues[0].code, "SPEECH_AUDIO_TRUNCATED");
    assert!(issues[0].asset_id.is_some());
    let track = p
        .tracks
        .iter_mut()
        .find(|t| t.id == p.clips[0].track_id)
        .unwrap();
    track.muted = true;
    assert!(review_audio_timing(&p).1.is_empty());
}
#[test]
fn nested_windows_preserve_longer_predecessor_and_missing_audio_is_explicit() {
    let mut p = project();
    add(&mut p, 200_000, 100_000);
    add(&mut p, 400_000, 1_000_000);
    let (_, issues) = review_audio_timing(&p);
    assert_eq!(
        issues.iter().filter(|i| i.code == "SPEECH_OVERLAP").count(),
        2
    );
    if let ClipContent::Speech { audio, .. } = &mut p.clips[0].content {
        *audio = None;
    }
    assert!(
        review_audio_timing(&p)
            .1
            .iter()
            .any(|i| i.code == "SPEECH_AUDIO_NOT_GENERATED")
    );
}
