use crate::{RenderPlan, ducking, ducking_tests::fixture};
use std::{ffi::OsString, fs, path::Path, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::{ProcessSpec, run_process};
use sungeki_project::{Ducking, TimeUs};

#[test]
#[ignore = "requires VVM_TEST_FFMPEG; maximum-size ducking expression"]
fn maximum_audio_count_expression_matches_common_envelope() {
    let binary = std::env::var("VVM_TEST_FFMPEG").expect("explicit FFmpeg path");
    let mut p = fixture(0, 50000, 50000, false);
    let template = p.clips[1].clone();
    p.clips.truncate(1);
    for i in 0..127 {
        let mut clip = template.clone();
        clip.id = uuid::Uuid::new_v4().to_string();
        clip.start_us = TimeUs(i * 120000);
        p.clips.push(clip);
    }
    let plan = RenderPlan::new(&p, |_| Ok(())).unwrap();
    assert_eq!(plan.scene.audio.len(), 128);
    let settings = Ducking {
        attenuation_db: -20.0,
        attack_us: TimeUs(50000),
        release_us: TimeUs(50000),
    };
    let curve = ducking::envelope(&plan, &settings);
    let filter = ducking::filter(&plan, TimeUs(0), &settings);
    let dir = std::env::temp_dir().join(format!("vvm-duck-limit-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    // Exercise all binary-search leaves without opening 127 redundant inputs.
    let args: Vec<OsString> = [
        "-nostdin",
        "-hide_banner",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "aevalsrc=0.25|-0.125:s=48000:d=16",
        "-af",
        filter.trim_end_matches(','),
        "-f",
        "f32le",
        "pipe:1",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    let output = run_process(
        &ProcessSpec {
            program: Path::new(&binary),
            args: &args,
            directory: &dir,
            timeout: Duration::from_secs(30),
            max_output_bytes: 8 * 1024 * 1024,
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(
        output.succeeded(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout.len(), 16 * 48000 * 8);
    for (i, pair) in output.stdout.chunks_exact(8).enumerate() {
        let gain = curve.gain_at(TimeUs(i as u64 * 1000000 / 48000));
        for (ch, b) in pair.chunks_exact(4).enumerate() {
            let actual = f32::from_le_bytes(b.try_into().unwrap()) as f64;
            let expected = gain * if ch == 0 { 0.25 } else { -0.125 };
            // Reference time is integer us; the filter keeps sub-us sample time.
            assert!(
                (actual - expected).abs() < 5e-6,
                "sample={i} channel={ch}: {actual} != {expected}"
            );
        }
    }
    fs::remove_dir_all(dir).unwrap();
}
