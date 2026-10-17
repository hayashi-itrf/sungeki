use std::cell::Cell;
use sungeki_project::*;
use sungeki_render::{RenderError, RenderPlan};

fn fixture() -> Project {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scene-evaluation.json"
    ))
    .unwrap();
    serde_json::from_value(value["project"].clone()).unwrap()
}
fn sound(project: &mut Project) {
    let id = "00000000-0000-4000-8000-000000000050".to_owned();
    let sha256 = "a".repeat(64);
    project.assets.push(Asset {
        id: id.clone(),
        name: "voice".into(),
        source: None,
        credit: "test".into(),
        locator: AssetLocator::Absolute {
            path: "/unused.wav".into(),
        },
        content_hash: Some(sha256.clone()),
        probe: AssetProbe::Audio(AudioProbe {
            encoding: None,
            sha256,
            duration_us: TimeUs(100000),
            sample_rate: 24000,
            channels: 1,
            frames: 2400,
            byte_length: 4844,
        }),
    });
    let clip = &mut project.clips[4];
    clip.duration_us = TimeUs(100000);
    let ClipContent::Speech { audio, .. } = &mut clip.content else {
        panic!()
    };
    *audio = Some(SpeechAudio {
        irodori_source: None,
        qwen_source: None,
        sbv2_source: None,
        asset_id: id,
        input_fingerprint: "b".repeat(64),
        engine_version: "test".into(),
        lip_sync: None,
    });
}

#[test]
fn missing_audio_is_rejected_even_when_track_is_invisible() {
    let project = fixture();
    assert!(
        RenderPlan::new(&project, |_| panic!("no audio to validate"))
            .unwrap_err()
            .to_string()
            .contains("未合成")
    );
}
#[test]
fn freshness_is_checked_on_the_audible_clip_and_errors_are_preserved() {
    let mut project = fixture();
    sound(&mut project);
    let before = project.clone();
    let calls = Cell::new(0);
    let plan = RenderPlan::new(&project, |id| {
        assert_eq!(id, project.clips[4].id);
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(plan.frame_count(), 7);
    assert_eq!(plan.audio_samples(), 11200);
    assert_eq!(project, before);
    assert_eq!(
        RenderPlan::new(&project, |_| Err(RenderError::Message("stale".into())))
            .unwrap_err()
            .to_string(),
        "stale"
    );
}
#[test]
fn no_silent_truncation_or_ignored_normalization() {
    let mut project = fixture();
    sound(&mut project);
    project.clips[4].duration_us = TimeUs(99999);
    assert!(
        RenderPlan::new(&project, |_| Ok(()))
            .unwrap_err()
            .to_string()
            .contains("尺を超え")
    );
    project.clips[4].duration_us = TimeUs(100000);
    project.audio_mix.normalization = Some(Normalization {
        target_lufs: -16.0,
        true_peak_db: -1.0,
    });
    assert!(RenderPlan::new(&project, |_| Ok(())).is_ok());
    project
        .audio_mix
        .normalization
        .as_mut()
        .unwrap()
        .target_lufs = -70.;
    assert!(
        RenderPlan::new(&project, |_| Ok(()))
            .unwrap_err()
            .to_string()
            .contains("測定下限")
    );
}
#[test]
fn muted_missing_speech_does_not_require_synthesis() {
    let mut project = fixture();
    project.tracks.iter_mut().for_each(|t| t.muted = true);
    assert!(RenderPlan::new(&project, |_| panic!("muted")).is_ok());
}
#[test]
fn duration_is_rounded_to_complete_frames_then_audio_samples() {
    let mut project = Project::empty(
        "00000000-0000-4000-8000-000000000001".into(),
        "fractional".into(),
    );
    project.canvas.fps = FrameRate {
        numerator: 30000,
        denominator: 1001,
    };
    project.scenes.push(Scene {
        fade: None,
        id: "00000000-0000-4000-8000-000000000002".into(),
        name: "empty".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(1_000_000),
    });
    let plan = RenderPlan::new(&project, |_| Ok(())).unwrap();
    assert_eq!(plan.frame_count(), 30);
    assert_eq!(plan.audio_samples(), 48048);
    project.canvas.width = 1919;
    assert!(RenderPlan::new(&project, |_| Ok(())).is_err());
    project.scenes.clear();
    assert!(RenderPlan::new(&project, |_| Ok(())).is_err());
}

#[test]
fn material_audio_checks_output_samples_and_total_loop_memory() {
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/audio-project.json")).unwrap();
    RenderPlan::new(&p, |_| panic!("material must not invoke speech freshness")).unwrap();
    let ClipContent::Audio { playback, .. } = &mut p.clips[0].content else {
        panic!()
    };
    playback.fade_in_us = TimeUs(0);
    playback.fade_out_us = TimeUs(0);
    p.clips[0].duration_us = TimeUs(1);
    assert!(
        RenderPlan::new(&p, |_| Ok(()))
            .unwrap_err()
            .to_string()
            .contains("1サンプル")
    );
    p.clips[0].duration_us = TimeUs(2_000_000);
    let AssetProbe::Audio(probe) = &mut p.assets[0].probe else {
        panic!()
    };
    probe.frames = 8_000_000;
    probe.byte_length = 16_000_044;
    probe.duration_us = TimeUs(166_666_667);
    let ClipContent::Audio { playback, .. } = &mut p.clips[0].content else {
        panic!()
    };
    playback.source_duration_us = TimeUs(100_000_000);
    for i in 0..7 {
        let mut clip = p.clips[0].clone();
        clip.id = format!("00000000-0000-4000-8000-{:012x}", 100 + i);
        p.clips.push(clip);
    }
    assert!(
        RenderPlan::new(&p, |_| Ok(()))
            .unwrap_err()
            .to_string()
            .contains("256MiB")
    );
}

#[test]
fn two_minute_animation_is_not_rejected_by_static_span_limit() {
    let mut project = fixture();
    project
        .clips
        .retain(|clip| matches!(clip.content, ClipContent::Shape { .. }));
    project.clips.truncate(1);
    assert_eq!(project.clips.len(), 1);
    let clip = &mut project.clips[0];
    clip.start_us = TimeUs(0);
    clip.duration_us = TimeUs(120_000_000);
    clip.transform.fade = Some(VisualFade {
        in_us: TimeUs(60_000_000),
        out_us: TimeUs(60_000_000),
    });
    let plan = RenderPlan::new(&project, |_| panic!("no speech")).unwrap();
    assert_eq!(plan.frame_count(), 3600);
    // Expansion is still bounded; accepting ordinary animation is not unbounded.
    project.clips[0].duration_us = TimeUs(4_000_000_000);
    project.clips[0].transform.fade = Some(VisualFade {
        in_us: TimeUs(2_000_000_000),
        out_us: TimeUs(2_000_000_000),
    });
    assert!(RenderPlan::new(&project, |_| Ok(())).is_err());
}
