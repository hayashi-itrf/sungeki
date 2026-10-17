#![cfg(unix)]
use std::{fs, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::video_probe::{ProbeRuntime, frames::PreparedVideo, probe};
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
#[ignore = "requires real FFmpeg/ffprobe and configured SSD root"]
fn video_origin_controls_audio_silence_trim_and_duration() {
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let ffprobe = PathBuf::from(std::env::var_os("VVM_TEST_FFPROBE").unwrap());
    let root = Root(
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
            .join(format!("video-audio-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&root.0).unwrap();
    let flag = AtomicBool::new(false);
    for (case, video_offset, audio_offset, first, last) in [
        (0, "0", "0.25", 12000, 36000),
        (1, "0.25", "0", 0, 12000),
        (2, "0.25", "0.25", 0, 24000),
    ] {
        let path = root.0.join(format!("case-{case}.mov"));
        let args: Vec<_> = [
            "-nostdin",
            "-v",
            "error",
            "-copyts",
            "-itsoffset",
            video_offset,
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=32x24:rate=4:duration=1",
            "-itsoffset",
            audio_offset,
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=0.5:sample_rate=48000:duration=0.5",
            "-c:v",
            "libx264",
            "-bf",
            "0",
            "-fps_mode",
            "passthrough",
            "-c:a",
            "pcm_s16le",
        ]
        .into_iter()
        .map(Into::into)
        .chain([path.as_os_str().to_owned()])
        .collect();
        let output = sungeki_media::run_process(
            &sungeki_media::ProcessSpec {
                program: &ffmpeg,
                args: &args,
                directory: &root.0,
                timeout: Duration::from_secs(30),
                max_output_bytes: 65536,
            },
            &flag,
        )
        .unwrap();
        assert!(output.succeeded(), "{output:?}");
        let original = fs::read(&path).unwrap();
        let runtime = ProbeRuntime {
            ffprobe: &ffprobe,
            root: &root.0,
            timeout: Duration::from_secs(30),
            max_source_bytes: 1048576,
        };
        let saved = probe(&path, &runtime, &flag).unwrap();
        let prepared = PreparedVideo::open(&path, &saved, &runtime, &flag).unwrap();
        let audio = prepared
            .decode_audio(
                saved.metadata.audio[0].index,
                &ffmpeg,
                Duration::from_secs(30),
                &flag,
            )
            .unwrap();
        assert_eq!(audio.info().frames, 48000);
        let mut wav = hound::WavReader::open(audio.path()).unwrap();
        assert_eq!(wav.spec().channels, 2);
        assert_eq!(wav.spec().sample_rate, 48000);
        let samples: Vec<f32> = wav.samples().map(Result::unwrap).collect();
        assert_eq!(samples.len(), 96000);
        for (index, frame) in samples.chunks_exact(2).enumerate() {
            assert_eq!(frame[0], frame[1]);
            if (first..last).contains(&index) {
                assert!(frame[0] > 0.1, "case {case} sample {index}: {}", frame[0]);
            } else {
                assert_eq!(frame[0], 0.0, "case {case} sample {index}");
            }
        }
        let owned = audio.path().to_owned();
        drop(audio);
        assert!(!owned.exists());
        assert!(
            prepared
                .decode_audio(99, &ffmpeg, Duration::from_secs(30), &flag)
                .is_err()
        );
        assert!(
            prepared
                .decode_audio(
                    saved.metadata.audio[0].index,
                    &ffmpeg,
                    Duration::from_secs(30),
                    &AtomicBool::new(true)
                )
                .is_err()
        );
        if case == 0 {
            use std::os::unix::fs::PermissionsExt;
            let worker = root.0.join("audio-worker");
            for script in [
                "printf partial",
                "echo broken >&2; exit 0",
                "sleep 5",
                r"printf '\000\000\300\177'; head -c 383996 /dev/zero",
            ] {
                fs::write(&worker, format!("#!/bin/sh\n{script}\n")).unwrap();
                fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
                let timeout = if script == "sleep 5" {
                    Duration::from_millis(100)
                } else {
                    Duration::from_secs(3)
                };
                assert!(
                    prepared
                        .decode_audio(saved.metadata.audio[0].index, &worker, timeout, &flag)
                        .is_err()
                );
                for directory in fs::read_dir(&root.0).unwrap() {
                    let directory = directory.unwrap().path();
                    if directory.is_dir() {
                        assert!(!fs::read_dir(directory).unwrap().any(|p| {
                            p.unwrap()
                                .file_name()
                                .to_string_lossy()
                                .starts_with("audio-")
                        }));
                    }
                }
            }
        }
        assert_eq!(fs::read(path).unwrap(), original);
        drop(prepared);
        assert!(!fs::read_dir(&root.0).unwrap().any(|p| {
            p.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("video-probe-")
        }));
    }
}
