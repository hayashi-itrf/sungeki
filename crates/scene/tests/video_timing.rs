use sungeki_project::{AssetProbe, ClipContent, Project, TimeUs};
use sungeki_scene::VideoClipTiming;
fn fixture() -> Project {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap()
}
#[test]
fn trim_loop_half_open_and_negative_source_origin() {
    let p = fixture();
    let mut clip = p.clips[0].clone();
    let AssetProbe::Video(probe) = &p.assets[0].probe else {
        panic!()
    };
    assert!(probe.metadata.video.clock.start_pts.unwrap() < 0);
    clip.start_us = TimeUs(1_000_000);
    clip.duration_us = TimeUs(600_000);
    let clock = VideoClipTiming::new(&clip, probe).unwrap();
    assert_eq!(clock.source_time(TimeUs(999_999)), None);
    assert_eq!(clock.source_time(TimeUs(1_000_000)), Some(TimeUs(200_000)));
    assert_eq!(clock.source_time(TimeUs(1_599_999)), Some(TimeUs(799_999)));
    assert_eq!(clock.source_time(TimeUs(1_600_000)), None);
    let ClipContent::Video { playback, .. } = &mut clip.content else {
        panic!()
    };
    playback.looped = true;
    clip.duration_us = TimeUs(2_000_000);
    let clock = VideoClipTiming::new(&clip, probe).unwrap();
    for (time, expected) in [
        (1_600_000, 200_000),
        (1_600_001, 200_001),
        (2_200_000, 200_000),
        (2_999_999, 399_999),
    ] {
        assert_eq!(clock.source_time(TimeUs(time)), Some(TimeUs(expected)));
    }
    assert_eq!(clock.source_time(TimeUs(3_000_000)), None);
    assert_eq!(clock.source_time(TimeUs(u64::MAX)), None);
}
#[test]
fn invalid_interval_never_panics_or_wraps() {
    let p = fixture();
    let AssetProbe::Video(probe) = &p.assets[0].probe else {
        panic!()
    };
    let mut c = p.clips[0].clone();
    c.start_us = TimeUs(u64::MAX);
    assert!(VideoClipTiming::new(&c, probe).is_err());
    c.start_us = TimeUs(0);
    let ClipContent::Video { playback, .. } = &mut c.content else {
        panic!()
    };
    playback.looped = true;
    playback.source_duration_us = TimeUs(0);
    assert!(VideoClipTiming::new(&c, probe).is_err());
}
