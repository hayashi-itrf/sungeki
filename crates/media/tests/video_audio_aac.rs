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
fn aac_resampling_preserves_decoded_samples_and_delayed_origin() {
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let ffprobe = PathBuf::from(std::env::var_os("VVM_TEST_FFPROBE").unwrap());
    let root = Root(
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
            .join(format!("video-aac-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&root.0).unwrap();
    let flag = AtomicBool::new(false);
    let run = |args: Vec<std::ffi::OsString>| {
        let output = sungeki_media::run_process(
            &sungeki_media::ProcessSpec {
                program: &ffmpeg,
                args: &args,
                directory: &root.0,
                timeout: Duration::from_secs(30),
                max_output_bytes: 2 * 1024 * 1024,
            },
            &flag,
        )
        .unwrap();
        assert!(
            output.succeeded(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    for (case, rate, offset) in [(0, "44100", "0.25"), (1, "48000", "0"), (2, "44100", "0")] {
        let path = root.0.join(format!("source-{case}.mp4"));
        let signal = format!("sine=frequency=997:sample_rate={rate}:duration=1");
        let args: Vec<_> = [
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=32x24:rate=4:duration=2",
            "-itsoffset",
            offset,
            "-f",
            "lavfi",
            "-i",
            &signal,
            "-c:v",
            "libx264",
            "-bf",
            "0",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
        ]
        .into_iter()
        .map(Into::into)
        .chain([path.as_os_str().to_owned()])
        .collect();
        run(args);
        let runtime = ProbeRuntime {
            ffprobe: &ffprobe,
            root: &root.0,
            timeout: Duration::from_secs(30),
            max_source_bytes: 1048576,
        };
        let saved = probe(&path, &runtime, &flag).unwrap();
        let audio_stream = &saved.metadata.audio[0];
        assert_eq!(audio_stream.codec, "aac");
        assert_eq!(audio_stream.sample_rate, rate.parse::<u32>().unwrap());
        if offset == "0" {
            let packets = std::process::Command::new(&ffprobe)
                .args([
                    "-v",
                    "error",
                    "-select_streams",
                    "a:0",
                    "-show_packets",
                    "-show_entries",
                    "packet=pts:packet_side_data=side_data_type,skip_samples",
                    "-of",
                    "json",
                ])
                .arg(&path)
                .output()
                .unwrap();
            assert!(packets.status.success());
            let packets: serde_json::Value = serde_json::from_slice(&packets.stdout).unwrap();
            assert!(
                packets["packets"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|packet| packet["pts"].as_i64().is_some_and(|pts| pts < 0)
                        && packet["side_data_list"]
                            .as_array()
                            .is_some_and(|entries| entries.iter().any(|entry| {
                                entry["skip_samples"].as_u64().is_some_and(|skip| skip > 0)
                            }))),
                "fixture must contain actual AAC encoder priming"
            );
        }
        let prepared = PreparedVideo::open(&path, &saved, &runtime, &flag).unwrap();
        let audio = prepared
            .decode_audio(audio_stream.index, &ffmpeg, Duration::from_secs(30), &flag)
            .unwrap();
        let actual: Vec<f32> = hound::WavReader::open(audio.path())
            .unwrap()
            .samples()
            .map(Result::unwrap)
            .collect();
        assert_eq!(actual.len(), 96000 * 2);
        let baseline = run(["-nostdin", "-v", "error", "-i"]
            .into_iter()
            .map(Into::into)
            .chain([path.as_os_str().to_owned()])
            .chain(["-map".into(), format!("0:{}", audio_stream.index).into()])
            .chain(
                [
                    "-vn",
                    "-af",
                    "aresample=48000",
                    "-ac",
                    "2",
                    "-c:a",
                    "pcm_f32le",
                    "-f",
                    "f32le",
                    "pipe:1",
                ]
                .into_iter()
                .map(Into::into),
            )
            .collect());
        let reference: Vec<f32> = baseline
            .chunks_exact(4)
            .map(|p| f32::from_le_bytes(p.try_into().unwrap()))
            .collect();
        let clock = &audio_stream.clock;
        let numerator =
            i128::from(clock.start_pts.unwrap()) * i128::from(clock.time_base.numerator) * 48000;
        let denominator = i128::from(clock.time_base.denominator);
        let start = ((numerator + denominator / 2) / denominator) as usize;
        assert!(actual[..start * 2].iter().all(|s| *s == 0.0));
        let max = actual[start * 2..start * 2 + reference.len()]
            .iter()
            .zip(&reference)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(
            max < 1e-6,
            "case {case}, start {start}, max difference {max}"
        );
        assert!(
            actual[start * 2 + reference.len()..]
                .iter()
                .all(|s| *s == 0.0)
        );
    }
}
