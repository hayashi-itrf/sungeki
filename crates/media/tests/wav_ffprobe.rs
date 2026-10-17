use std::{collections::BTreeMap, fs, io::Cursor, process::Command};
use sungeki_media::{ExpectedAudio, store_wav_new, verify_wav};

#[test]
#[ignore = "requires ffprobe and VVM_MEDIA_TEST_ROOT on the configured external SSD"]
fn saved_wav_metadata_agrees_with_independent_ffprobe() {
    let root = std::path::PathBuf::from(
        std::env::var_os("VVM_MEDIA_TEST_ROOT").expect("explicit test root"),
    )
    .join(format!("wav-probe-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    for (channels, rate, frames) in [(1, 44100, 1), (2, 48000, 480)] {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(
                &mut bytes,
                hound::WavSpec {
                    channels,
                    sample_rate: rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for _ in 0..channels as u32 * frames {
                writer.write_sample(1i16).unwrap();
            }
            writer.finalize().unwrap();
        }
        let audio = verify_wav(
            bytes.into_inner(),
            ExpectedAudio {
                channels,
                sample_rate: rate,
            },
        )
        .unwrap();
        let destination = root.join(format!("{channels}-{rate}.wav"));
        store_wav_new(&audio, &destination).unwrap();
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "a:0",
                "-show_entries",
                "stream=sample_rate,channels,duration_ts,time_base",
                "-of",
                "default=noprint_wrappers=1",
            ])
            .arg(&destination)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        let fields: BTreeMap<_, _> = text
            .lines()
            .filter_map(|line| line.split_once('='))
            .collect();
        assert_eq!(
            fields["sample_rate"].parse::<u32>().unwrap(),
            audio.info().sample_rate
        );
        assert_eq!(
            fields["channels"].parse::<u16>().unwrap(),
            audio.info().channels
        );
        assert_eq!(
            fields["duration_ts"].parse::<u64>().unwrap(),
            audio.info().frames
        );
        assert_eq!(fields["time_base"], format!("1/{rate}"));
        println!(
            "{channels}ch {rate}Hz: ffprobe={frames} frames; clip={}us",
            audio.info().duration_us
        );
    }
    fs::remove_dir_all(root).unwrap();
}
