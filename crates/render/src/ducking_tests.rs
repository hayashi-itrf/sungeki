use crate::RenderPlan;
use std::{ffi::OsString, fs, path::Path, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::{ProcessSpec, run_process};
use sungeki_project::*;

pub(super) fn fixture(start: u64, attack: u64, release: u64, muted: bool) -> Project {
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/audio-project.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scene-evaluation.json"
    ))
    .unwrap();
    let speech: Project = serde_json::from_value(value["project"].clone()).unwrap();
    p.characters = speech.characters;
    p.clips[0].start_us = TimeUs(start);
    let ClipContent::Audio { playback, .. } = &mut p.clips[0].content else {
        panic!()
    };
    playback.ducking = Some(Ducking {
        attenuation_db: -20.0,
        attack_us: TimeUs(attack),
        release_us: TimeUs(release),
    });
    let AssetProbe::Audio(probe) = &mut p.assets[0].probe else {
        panic!()
    };
    probe.channels = 2;
    probe.byte_length = 192044;
    let mut asset = p.assets[0].clone();
    asset.id = uuid::Uuid::new_v4().to_string();
    let AssetProbe::Audio(probe) = &mut asset.probe else {
        panic!()
    };
    probe.frames = 4800;
    probe.duration_us = TimeUs(100000);
    probe.byte_length = 19244;
    let mut track = p.tracks[0].clone();
    track.id = uuid::Uuid::new_v4().to_string();
    track.visible = false;
    track.muted = muted;
    for at in [300000, 350000, 900000] {
        let mut clip = speech.clips[4].clone();
        clip.id = uuid::Uuid::new_v4().to_string();
        clip.track_id = track.id.clone();
        clip.start_us = TimeUs(at);
        clip.duration_us = TimeUs(600000); // subtitle tail must NOT drive ducking
        let ClipContent::Speech { audio, .. } = &mut clip.content else {
            panic!()
        };
        *audio = Some(SpeechAudio {
            irodori_source: None,
            qwen_source: None,
            sbv2_source: None,
            asset_id: asset.id.clone(),
            input_fingerprint: "b".repeat(64),
            engine_version: "fixture".into(),
            lip_sync: None,
        });
        p.clips.push(clip);
    }
    p.tracks.push(track);
    p.assets.push(asset);
    p
}

fn wav(frames: u32, value: i16) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(b"RIFF");
    b.extend((36 + frames * 4).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(48000u32.to_le_bytes());
    b.extend(192000u32.to_le_bytes());
    b.extend(4u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend((frames * 4).to_le_bytes());
    for _ in 0..frames {
        b.extend(value.to_le_bytes());
        b.extend((-value / 2).to_le_bytes());
    }
    b
}

#[test]
fn windows_use_actual_speech_duration_and_exclude_muted_tracks() {
    let p = fixture(100003, 100000, 200000, false);
    let plan = RenderPlan::new(&p, |_| Ok(())).unwrap();
    let settings = Ducking {
        attenuation_db: -20.0,
        attack_us: TimeUs(100000),
        release_us: TimeUs(200000),
    };
    let envelope = super::ducking::envelope(&plan, &settings);
    assert_eq!(
        envelope.windows().collect::<Vec<_>>(),
        vec![
            (TimeUs(300000), TimeUs(450000)),
            (TimeUs(900000), TimeUs(1000000))
        ]
    );
    let plan = RenderPlan::new(&fixture(0, 0, 0, true), |_| panic!("muted")).unwrap();
    assert_eq!(
        super::ducking::envelope(&plan, &settings).windows().count(),
        0
    );
    assert!(RenderPlan::new(&p, |_| Err(crate::RenderError::Message("stale".into()))).is_err());
}

#[test]
#[ignore = "requires VVM_TEST_FFMPEG; real ducked PCM comparison"]
fn real_ducking_after_gain_fade_loop_and_before_mix_matches_pcm() {
    let binary = std::env::var("VVM_TEST_FFMPEG").expect("explicit FFmpeg path");
    let dir = std::env::temp_dir().join(format!("vvm-ducking-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("bgm.wav"), wav(48000, 8192)).unwrap();
    fs::write(dir.join("speech.wav"), wav(4800, 0)).unwrap();
    for (start, attack, release, muted, points) in [
        (0, 100000, 200000, false, false),
        (100003, 100000, 200000, false, false),
        (100003, 0, 0, false, false),
        (0, 100000, 200000, true, false),
        (0, 100000, 200000, false, true),
        (100003, 100000, 200000, false, true),
        (100003, 0, 0, false, true),
        (0, 100000, 200000, true, true),
    ] {
        let mut project = fixture(start, attack, release, muted);
        if points {
            let ClipContent::Audio { playback, .. } = &mut project.clips[0].content else {
                panic!()
            };
            playback.volume_points = [(0, 0.25), (600000, 1.0), (1100000, 0.0), (1700000, 1.0)]
                .into_iter()
                .map(|(at_us, level)| VolumePoint {
                    at_us: TimeUs(at_us),
                    level,
                })
                .collect();
        }
        let plan = RenderPlan::new(&project, |_| Ok(())).unwrap();
        fs::write(dir.join("audio.filter"), plan.audio_graph()).unwrap();
        let mut args: Vec<OsString> = [
            "-nostdin",
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=s=2x2:d=0.1",
            "-i",
            "bgm.wav",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        for _ in 1..plan.scene.audio.len() {
            args.extend(["-i".into(), "speech.wav".into()]);
        }
        args.extend(
            [
                "-filter_complex_script",
                "audio.filter",
                "-map",
                "[audio]",
                "-f",
                "f32le",
                "-c:a",
                "pcm_f32le",
                "pipe:1",
            ]
            .into_iter()
            .map(OsString::from),
        );
        let output = run_process(
            &ProcessSpec {
                program: Path::new(&binary),
                args: &args,
                directory: &dir,
                timeout: Duration::from_secs(30),
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
        let first = ((start * 48000 + 500000) / 1000000) as usize;
        for (frame, stereo) in output.stdout.chunks_exact(8).enumerate() {
            let t = frame as f64 * 1000000.0 / 48000.0;
            let weight = if muted {
                0.0
            } else {
                [300000.0, 350000.0, 900000.0]
                    .into_iter()
                    .map(|s| {
                        if t < s {
                            if attack == 0 {
                                0.0
                            } else {
                                (1.0 - (s - t) / attack as f64).max(0.0)
                            }
                        } else if t < s + 100000.0 {
                            1.0
                        } else if release == 0 {
                            0.0
                        } else {
                            (1.0 - (t - s - 100000.0) / release as f64).max(0.0)
                        }
                    })
                    .fold(0.0, f64::max)
            };
            let expected = if frame < first || frame >= first + 96000 {
                0.0
            } else {
                let i = frame - first;
                let local = i as f64 / 48000.0;
                let point_gain = if !points {
                    1.0
                } else if local < 0.6 {
                    0.25 + local / 0.6 * 0.75
                } else if local < 1.1 {
                    1.0 - (local - 0.6) / 0.5
                } else if local < 1.7 {
                    (local - 1.1) / 0.6
                } else {
                    1.0
                };
                0.25 * point_gain
                    * 10f64.powf(-18.0 / 20.0)
                    * (i as f64 / 4800.0).min(1.0)
                    * ((96000 - i) as f64 / 9600.0).min(1.0)
                    * (1.0 - 0.9 * weight)
            };
            for (ch, sample) in stereo.chunks_exact(4).enumerate() {
                let actual = f32::from_le_bytes(sample.try_into().unwrap()) as f64;
                let expected = expected * if ch == 0 { 1.0 } else { -0.5 };
                assert!(
                    (actual - expected).abs() < 1e-6,
                    "start={start} attack={attack} release={release} muted={muted} frame={frame} ch={ch}: {actual} != {expected}"
                );
            }
        }
    }
    fs::remove_dir_all(dir).unwrap();
}
