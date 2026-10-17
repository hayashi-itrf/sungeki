use sungeki_media::WavInfo;
use sungeki_project::{AssetProbe, Project, TimeUs, VideoAudioStreamInfo, VideoPlayback};
use sungeki_render::VideoAudioSelection;
fn fixture() -> (sungeki_project::VideoProbe, WavInfo, VideoPlayback) {
    let p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json")).unwrap();
    let AssetProbe::Video(mut probe) = p.assets[0].probe.clone() else {
        panic!()
    };
    probe.metadata.audio.push(VideoAudioStreamInfo {
        index: 1,
        codec: "aac".into(),
        sample_rate: 44100,
        channels: 1,
        clock: probe.metadata.video.clock.clone(),
    });
    let wav = WavInfo {
        sample_rate: 48000,
        channels: 2,
        bits_per_sample: 32,
        is_float: true,
        frames: 48000,
        duration_us: 1_000_000,
        content_hash: "a".repeat(64),
    };
    let playback = VideoPlayback {
        source_start_us: TimeUs(250000),
        source_duration_us: TimeUs(250000),
        looped: true,
        audio_stream: Some(1),
        gain_db: -6.020599913279624,
    };
    (probe, wav, playback)
}
#[test]
fn invalid_audio_clock_and_empty_intervals_are_rejected() {
    let (probe, mut wav, mut playback) = fixture();
    assert!(VideoAudioSelection::new(&playback, &probe, &wav, TimeUs(0), TimeUs(750000)).is_ok());
    wav.frames -= 1;
    assert!(VideoAudioSelection::new(&playback, &probe, &wav, TimeUs(0), TimeUs(750000)).is_err());
    wav.frames += 1;
    playback.source_start_us = TimeUs(1);
    playback.source_duration_us = TimeUs(1);
    assert!(VideoAudioSelection::new(&playback, &probe, &wav, TimeUs(0), TimeUs(750000)).is_err());
}
#[test]
#[ignore = "requires real FFmpeg and SSD test root"]
fn actual_filter_trims_loops_scales_and_delays_samples() {
    use std::{fs, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
    let (probe, wav, playback) = fixture();
    let selection =
        VideoAudioSelection::new(&playback, &probe, &wav, TimeUs(100000), TimeUs(750000)).unwrap();
    let root = PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
        .join(format!("video-selection-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    // Canonical WAV bytes from the shared, verified 48kHz/stereo representation.
    let mut pcm = Vec::new();
    for i in 0..48000 {
        let v = (i % 1000) as f32 / 2000.0;
        pcm.extend(v.to_le_bytes());
        pcm.extend((-v).to_le_bytes());
    }
    let path = root.join("input.f32");
    fs::write(&path, pcm).unwrap();
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let filter = selection.filter();
    let args: Vec<_> = [
        "-nostdin", "-v", "error", "-f", "f32le", "-ar", "48000", "-ac", "2", "-i",
    ]
    .into_iter()
    .map(Into::into)
    .chain([path.as_os_str().to_owned()])
    .chain(
        ["-af", &filter, "-f", "f32le", "-c:a", "pcm_f32le", "pipe:1"]
            .into_iter()
            .map(Into::into),
    )
    .collect();
    let out = sungeki_media::run_process(
        &sungeki_media::ProcessSpec {
            program: &ffmpeg,
            args: &args,
            directory: &root,
            timeout: Duration::from_secs(30),
            max_output_bytes: 1024 * 1024,
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(out.succeeded(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(out.stdout.len(), (4800 + 36000) * 8);
    for (i, bytes) in out.stdout.chunks_exact(8).enumerate() {
        let left = f32::from_le_bytes(bytes[..4].try_into().unwrap());
        let right = f32::from_le_bytes(bytes[4..].try_into().unwrap());
        let expected = if i < 4800 {
            0.0
        } else {
            ((12000 + (i - 4800) % 12000) % 1000) as f32 / 4000.0
        };
        assert!((left - expected).abs() < 1e-7, "sample {i}");
        assert_eq!(left, -right);
    }
}
