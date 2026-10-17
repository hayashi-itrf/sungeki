use serde_json::json;
use sungeki_project::{MAX_TIME_US, MediaRatio, MediaStreamClock, VideoProbe};
fn fixture() -> VideoProbe {
    serde_json::from_value(json!({
        "sha256":"a".repeat(64),"byte_length":1024,
        "metadata":{"container":"mov","video":{"index":0,"codec":"h264","width":720,"height":480,
            "sample_aspect_ratio":{"numerator":8,"denominator":9},
            "clock":{"time_base":{"numerator":1,"denominator":90000},"start_pts":-9000,"duration_ts":3003},
            "average_frame_rate":{"numerator":30000,"denominator":1001},
            "nominal_frame_rate":{"numerator":30000,"denominator":1001},
            "frame_count":1,"rotation_degrees":90,"display_matrix":"matrix retained for later interpretation","legacy_rotate_tag":null},
            "audio":[{"index":1,"codec":"aac","sample_rate":48000,"channels":2,
                "clock":{"time_base":{"numerator":1,"denominator":48000},"start_pts":1024,"duration_ts":48000}}]}
    })).unwrap()
}
#[test]
fn source_identity_and_independent_signed_clocks_survive_round_trip() {
    let p = fixture();
    p.validate().unwrap();
    let loaded: VideoProbe = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded, p);
    assert_eq!(
        loaded
            .metadata
            .video
            .clock
            .duration_us()
            .unwrap()
            .unwrap()
            .0,
        33367
    );
    assert_eq!(
        loaded.metadata.audio[0]
            .clock
            .duration_us()
            .unwrap()
            .unwrap()
            .0,
        1_000_000
    );
    assert_eq!(loaded.metadata.video.clock.start_pts, Some(-9000));
}
#[test]
fn integer_precision_zero_ratio_and_conflicting_streams_are_rejected() {
    let mutate: [fn(&mut VideoProbe); 9] = [
        |p| p.metadata.video.clock.start_pts = Some(i64::MIN),
        |p| p.metadata.video.clock.duration_ts = Some(9_007_199_254_740_992),
        |p| p.metadata.video.frame_count = Some(9_007_199_254_740_992),
        |p| p.metadata.video.clock.time_base.denominator = 0,
        |p| p.metadata.audio[0].index = 0,
        |p| p.metadata.video.rotation_degrees = Some(f64::NAN),
        |p| p.metadata.video.display_matrix = Some("x".repeat(4097)),
        |p| p.byte_length = 4 * 1024 * 1024 * 1024 + 1,
        |p| p.sha256 = "Z".repeat(64),
    ];
    for change in mutate {
        let mut p = fixture();
        change(&mut p);
        assert!(p.validate().is_err());
    }
    let mut p = fixture();
    p.metadata.audio.push(p.metadata.audio[0].clone());
    assert!(p.validate().is_err());
}
#[test]
fn unknown_and_overflowing_duration_are_not_silently_clamped() {
    let mut clock = MediaStreamClock {
        time_base: MediaRatio {
            numerator: 1,
            denominator: 1_000_000,
        },
        start_pts: None,
        duration_ts: None,
    };
    assert_eq!(clock.duration_us().unwrap(), None);
    clock.duration_ts = Some(MAX_TIME_US);
    assert_eq!(clock.duration_us().unwrap().unwrap().0, MAX_TIME_US);
    clock.duration_ts = Some(MAX_TIME_US + 1);
    assert!(clock.duration_us().is_err());
    clock.duration_ts = Some(9_007_199_254_740_991);
    clock.time_base.numerator = u32::MAX;
    assert!(clock.duration_us().is_err());
    clock.duration_ts = Some(0);
    assert!(clock.validate().is_err());
}
#[test]
fn unknown_persisted_fields_fail_instead_of_disappearing() {
    let mut value = serde_json::to_value(fixture()).unwrap();
    value["metadata"]["video"]["proxy_path"] = json!("/not-an-original");
    assert!(serde_json::from_value::<VideoProbe>(value).is_err());
}
