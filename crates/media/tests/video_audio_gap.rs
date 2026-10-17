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
fn short_packet_gap_is_silence_not_a_time_shift() {
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let ffprobe = PathBuf::from(std::env::var_os("VVM_TEST_FFPROBE").unwrap());
    let root = Root(
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
            .join(format!("video-gap-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&root.0).unwrap();
    for gap in [2400, 24, 1] {
        let path = root.0.join(format!("gap-{gap}.mp4"));
        let filter = format!("asetnsamples=n=4608:p=0,asetpts=PTS+gte(T\\,0.192)*{gap}");
        let flag = AtomicBool::new(false);
        let args: Vec<_> = [
            "-nostdin",
            "-v",
            "error",
            "-copyts",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=32x24:rate=4:duration=1",
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=0.5:sample_rate=48000:duration=0.5",
            "-af",
            &filter,
            "-c:v",
            "libx264",
            "-bf",
            "0",
            "-c:a",
            "flac",
            "-strict",
            "-2",
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
        let packets = std::process::Command::new(&ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "a",
                "-show_frames",
                "-show_entries",
                "frame=pts,nb_samples",
                "-of",
                "json",
            ])
            .arg(&path)
            .output()
            .unwrap();
        assert!(packets.status.success());
        let frames: serde_json::Value = serde_json::from_slice(&packets.stdout).unwrap();
        let frames = frames["frames"].as_array().unwrap();
        let gaps: Vec<_> = frames
            .windows(2)
            .filter_map(|w| {
                let end = w[0]["pts"].as_i64().unwrap() + w[0]["nb_samples"].as_i64().unwrap();
                let next = w[1]["pts"].as_i64().unwrap();
                (end != next).then_some((end, next))
            })
            .collect();
        assert_eq!(
            gaps,
            vec![(9216, 9216 + gap)],
            "fixture must retain its gap after muxing"
        );
        let runtime = ProbeRuntime {
            ffprobe: &ffprobe,
            root: &root.0,
            timeout: Duration::from_secs(30),
            max_source_bytes: 1048576,
        };
        let probe = probe(&path, &runtime, &flag).unwrap();
        let video = PreparedVideo::open(&path, &probe, &runtime, &flag).unwrap();
        let audio = video
            .decode_audio(
                probe.metadata.audio[0].index,
                &ffmpeg,
                Duration::from_secs(30),
                &flag,
            )
            .unwrap();
        let samples: Vec<f32> = hound::WavReader::open(audio.path())
            .unwrap()
            .samples()
            .map(Result::unwrap)
            .collect();
        assert_eq!(samples.len(), 96000);
        let active: Vec<_> = samples
            .chunks_exact(2)
            .enumerate()
            .filter(|(_, p)| p[0] > 0.1)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(active.len(), 24000);
        for (i, frame) in samples.chunks_exact(2).enumerate() {
            let expected = (0..9216).contains(&i)
                || ((9216 + gap as usize)..(24000 + gap as usize)).contains(&i);
            assert_eq!(frame[0] > 0.1, expected, "sample {i}");
        }
    }
}
