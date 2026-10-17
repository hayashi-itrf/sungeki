use crate::RenderPlan;
use std::{ffi::OsString, fs, path::Path, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::{ProcessSpec, run_process};
use sungeki_project::{ClipContent, Project, TimeUs};

#[test]
#[ignore = "requires VVM_TEST_FFMPEG and VVM_TEST_FFPROBE; real frozen mix verification"]
fn real_loop_trim_gain_fade_delay_and_mix_match_every_sample() {
    let binary = std::env::var("VVM_TEST_FFMPEG").expect("explicit FFmpeg path");
    let dir = std::env::temp_dir().join(format!("vvm-audio-graph-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let mut project: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/audio-project.json")).unwrap();
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((36u32 + 48000 * 4).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(48000u32.to_le_bytes());
    bytes.extend(192000u32.to_le_bytes());
    bytes.extend(4u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend((48000u32 * 4).to_le_bytes());
    for i in 0..48000 {
        let value: i16 = if i < 4800 {
            24576
        } else if (i - 4800) % 24000 < 12000 {
            8192
        } else {
            16384
        };
        bytes.extend(value.to_le_bytes());
        bytes.extend((-value / 2).to_le_bytes());
    }
    fs::write(dir.join("source.wav"), bytes).unwrap();
    let sungeki_project::AssetProbe::Audio(probe) = &mut project.assets[0].probe else {
        panic!()
    };
    probe.channels = 2;
    probe.byte_length = 192044;
    for looping in [true, false] {
        project.clips.truncate(1);
        project.clips[0].start_us = TimeUs(100000);
        project.clips[0].duration_us = TimeUs(if looping { 2000000 } else { 500000 });
        let ClipContent::Audio { playback, .. } = &mut project.clips[0].content else {
            panic!()
        };
        playback.looped = looping;
        // A second overlapping cue checks that amix does not normalize inputs.
        let mut second = project.clips[0].clone();
        second.id = "00000000-0000-4000-8000-000000000099".into();
        project.clips.push(second);
        let mut plan = RenderPlan::new(&project, |_| panic!("material is not speech")).unwrap();
        let work = dir.join(if looping { "loop" } else { "single" });
        fs::create_dir(&work).unwrap();
        let runtime = crate::RenderRuntime {
            vrm: None,
            python: "/unused".into(),
            image_worker: "/unused".into(),
            ffmpeg: binary.clone().into(),
            ffprobe: std::env::var("VVM_TEST_FFPROBE")
                .expect("explicit ffprobe path")
                .into(),
            font: "/unused".into(),
            font_sha256: "unused".into(),
            deadline: Duration::from_secs(20),
        };
        let flag = AtomicBool::new(false);
        let runner = crate::execute::Runner {
            runtime: (&runtime).into(),
            directory: &work,
            flag: &flag,
            end: std::time::Instant::now() + runtime.deadline,
        };
        let resources = plan
            .sources
            .keys()
            .map(|id| {
                (
                    id.clone(),
                    serde_json::json!({"path": dir.join("source.wav")}),
                )
            })
            .collect();
        crate::audio_mix::freeze(&plan, &runner, &resources).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(work.join("mix.json")).unwrap()).unwrap();
        assert_eq!(manifest["frames"], plan.audio_samples());
        use sha2::{Digest, Sha256};
        let frozen = fs::read(work.join("mix.wav")).unwrap();
        assert_eq!(manifest["sha256"], format!("{:x}", Sha256::digest(&frozen)));
        // A second call cannot overwrite an already frozen input.
        assert!(crate::audio_mix::freeze(&plan, &runner, &resources).is_err());
        assert_eq!(fs::read(work.join("mix.wav")).unwrap(), frozen);
        let args: Vec<OsString> = [
            "-nostdin",
            "-hide_banner",
            "-v",
            "error",
            "-i",
            work.join("mix.wav").to_str().unwrap(),
            "-map",
            "0:a:0",
            "-f",
            "f32le",
            "-c:a",
            "pcm_f32le",
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
                timeout: Duration::from_secs(20),
                max_output_bytes: 2 * 1024 * 1024,
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(
            output.succeeded(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), plan.audio_samples() as usize * 8);
        let count = if looping { 96000 } else { 24000 };
        for (frame, stereo) in output.stdout.chunks_exact(8).enumerate() {
            let expected = if frame < 4800 || frame >= 4800 + count {
                0.0
            } else {
                let i = frame - 4800;
                let source = if i % 24000 < 12000 { 0.25 } else { 0.5 };
                let fade_in = (i as f64 / 4800.0).min(1.0);
                let fade_out = ((count - i) as f64 / 9600.0).min(1.0);
                2.0 * source * 10f64.powf(-18.0 / 20.0) * fade_in * fade_out
            };
            for (channel, sample) in stereo.chunks_exact(4).enumerate() {
                let actual = f32::from_le_bytes(sample.try_into().unwrap()) as f64;
                let expected = expected * if channel == 0 { 1.0 } else { -0.5 };
                assert!(
                    (actual - expected).abs() < 1e-6,
                    "loop={looping} frame={frame} channel={channel}: {actual} vs {expected}"
                );
            }
        }
        // No audible cue still yields the exact full-duration silent mix.
        plan.scene.audio.clear();
        let silent_work = work.join("silent");
        fs::create_dir(&silent_work).unwrap();
        let runner = crate::execute::Runner {
            directory: &silent_work,
            ..runner
        };
        crate::audio_mix::freeze(&plan, &runner, &resources).unwrap();
        let mut silent_args = args.clone();
        silent_args[5] = silent_work.join("mix.wav").into_os_string();
        let silence = run_process(
            &ProcessSpec {
                program: Path::new(&binary),
                args: &silent_args,
                directory: &silent_work,
                timeout: Duration::from_secs(20),
                max_output_bytes: 2 * 1024 * 1024,
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(
            silence.succeeded(),
            "{}",
            String::from_utf8_lossy(&silence.stderr)
        );
        assert_eq!(silence.stdout.len(), plan.audio_samples() as usize * 8);
        assert!(silence.stdout.iter().all(|&b| b == 0));
    }
    fs::remove_dir_all(dir).unwrap();
}
