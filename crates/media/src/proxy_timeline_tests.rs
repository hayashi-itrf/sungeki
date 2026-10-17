use super::*;
use sungeki_project::MediaRatio;

fn clock(origin: i64, duration: u64, denominator: u32) -> MediaStreamClock {
    MediaStreamClock {
        start_pts: Some(origin),
        duration_ts: Some(duration),
        time_base: MediaRatio {
            numerator: 1,
            denominator,
        },
    }
}

#[test]
fn vfr_rebased_clock_retains_every_cut_and_leading_gap() {
    let map = ProxyTimeline::verify(
        &clock(-9000, 18000, 90000),
        &[-8100, -6000, 0, 6000],
        &clock(0, 600, 3000),
        &[30, 100, 300, 500],
    )
    .unwrap();
    for (time, expected) in [
        (0, None),
        (9999, None),
        (10000, Some(30)),
        (33333, Some(30)),
        (33334, Some(100)),
        (99999, Some(100)),
        (100000, Some(300)),
        (166666, Some(300)),
        (166667, Some(500)),
        (199999, Some(500)),
    ] {
        assert_eq!(
            map.frame_at(TimeUs(time)).unwrap().map(|f| f.proxy_pts),
            expected
        );
    }
    assert_eq!(map.frames().len(), 4);
    assert!(map.frame_at(TimeUs(200000)).is_err());
    assert!(map.frame_at(TimeUs(u64::MAX)).is_err());
}

#[test]
fn timing_loss_and_incomplete_timelines_are_rejected() {
    let source = clock(0, 100, 1000);
    for pts in [
        vec![],
        vec![0, 30],
        vec![0, 30, 30],
        vec![0, 60, 30],
        vec![0, 31, 60],
        vec![0, 30, 100],
    ] {
        assert!(ProxyTimeline::verify(&source, &[0, 30, 60], &source, &pts).is_err());
    }
    for invalid in [
        clock(0, 101, 1000),
        clock(0, 0, 1000),
        clock(0, 100, 0),
        MediaStreamClock {
            start_pts: None,
            ..source.clone()
        },
        MediaStreamClock {
            duration_ts: None,
            ..source.clone()
        },
    ] {
        assert!(ProxyTimeline::verify(&source, &[0, 30, 60], &invalid, &[0, 30, 60]).is_err());
    }
    assert!(ProxyTimeline::verify(&source, &[0, 30, 60], &clock(0, 3, 30), &[0, 1, 2]).is_err());
}

#[test]
fn negative_preroll_and_large_timestamps_do_not_round_or_panic() {
    let source = clock(-100, 200, 1000);
    let map = ProxyTimeline::verify(&source, &[-150, 0], &source, &[-150, 0]).unwrap();
    assert_eq!(map.frame_at(TimeUs(0)).unwrap().unwrap().source_pts, -150);
    let extreme = MediaStreamClock {
        time_base: MediaRatio {
            numerator: u32::MAX,
            denominator: u32::MAX,
        },
        ..clock(i64::MIN, u64::MAX, 1)
    };
    assert!(ProxyTimeline::verify(&extreme, &[0], &extreme, &[0]).is_err());
    assert!(ProxyTimeline::verify(&source, &vec![0; 250_001], &source, &[0]).is_err());
}
