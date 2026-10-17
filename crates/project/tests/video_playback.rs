use sungeki_project::*;
fn project() -> Project {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap()
}
fn playback(p: &mut Project) -> &mut VideoPlayback {
    let ClipContent::Video { playback, .. } = &mut p.clips[0].content else {
        panic!()
    };
    playback
}
#[test]
fn trimmed_looped_and_muted_video_preserves_its_own_origin() {
    let mut p = project();
    validate(&p).unwrap();
    assert_eq!(
        serde_json::from_slice::<Project>(&serde_json::to_vec(&p).unwrap()).unwrap(),
        p
    );
    p.clips[0].duration_us = TimeUs(2_000_000);
    assert!(validate(&p).is_err());
    playback(&mut p).looped = true;
    validate(&p).unwrap();
    let AssetProbe::Video(probe) = &mut p.assets[0].probe else {
        panic!()
    };
    assert_eq!(probe.metadata.video.clock.start_pts, Some(-3000));
    probe.metadata.audio.push(VideoAudioStreamInfo {
        index: 1,
        codec: "aac".into(),
        sample_rate: 48000,
        channels: 2,
        clock: MediaStreamClock {
            time_base: MediaRatio {
                numerator: 1,
                denominator: 48000,
            },
            start_pts: Some(1024),
            duration_ts: Some(48000),
        },
    });
    playback(&mut p).audio_stream = Some(1);
    validate(&p).unwrap();
    playback(&mut p).audio_stream = Some(2);
    assert!(validate(&p).is_err());
}
#[test]
fn missing_clocks_wrong_track_and_invalid_ranges_are_rejected() {
    let mutations: [fn(&mut Project); 8] = [
        |p| playback(p).source_start_us = TimeUs(u64::MAX),
        |p| playback(p).source_duration_us = TimeUs(0),
        |p| playback(p).source_duration_us = TimeUs(800001),
        |p| playback(p).gain_db = f64::NAN,
        |p| p.tracks[0].kind = TrackKind::Audio,
        |p| {
            if let AssetProbe::Video(v) = &mut p.assets[0].probe {
                v.metadata.video.clock.duration_ts = None;
            }
        },
        |p| {
            if let AssetProbe::Video(v) = &mut p.assets[0].probe {
                v.metadata.video.clock.start_pts = None;
            }
        },
        |p| p.assets.clear(),
    ];
    for change in mutations {
        let mut p = project();
        change(&mut p);
        assert!(validate(&p).is_err());
    }
}
