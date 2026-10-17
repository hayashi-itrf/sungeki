use sungeki_project::{MAX_TIME_US, TimeUs};
use sungeki_scene::DuckingEnvelope;

fn envelope(windows: &[(u64, u64)], attack: u64, release: u64) -> DuckingEnvelope {
    DuckingEnvelope::new(
        windows.iter().map(|&(a, b)| (TimeUs(a), TimeUs(b))),
        -20.0,
        TimeUs(attack),
        TimeUs(release),
    )
    .unwrap()
}
fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
}

#[test]
fn anticipates_even_short_speech_and_releases_after_end() {
    let e = envelope(&[(1000, 1010)], 100, 200);
    for (t, gain) in [
        (899, 1.0),
        (900, 1.0),
        (950, 0.55),
        (1000, 0.1),
        (1009, 0.1),
        (1010, 0.1),
        (1110, 0.55),
        (1210, 1.0),
    ] {
        near(e.gain_at(TimeUs(t)), gain);
    }
}

#[test]
fn zero_boundaries_and_empty_windows_are_explicit() {
    let e = envelope(&[(0, 10), (20, 30)], 0, 0);
    for (t, gain) in [
        (0, 0.1),
        (9, 0.1),
        (10, 1.0),
        (19, 1.0),
        (20, 0.1),
        (30, 1.0),
    ] {
        near(e.gain_at(TimeUs(t)), gain);
    }
    near(envelope(&[], 100, 200).gain_at(TimeUs(0)), 1.0);
    near(envelope(&[(5, 10)], 10, 0).gain_at(TimeUs(0)), 0.55);
}

#[test]
fn overlapping_and_touching_speech_never_double_duck() {
    let e = envelope(&[(20, 40), (0, 30), (20, 40), (40, 50)], 10, 10);
    for t in 0..50 {
        near(e.gain_at(TimeUs(t)), 0.1);
    }
    near(e.gain_at(TimeUs(55)), 0.55);
    // A gap uses the stronger of release and upcoming attack, not their product.
    let e = envelope(&[(10, 20), (30, 40)], 10, 10);
    near(e.gain_at(TimeUs(25)), 0.55);
}

#[test]
fn optimized_lookup_matches_independent_all_window_reference() {
    let windows: Vec<_> = (0..128u64)
        .map(|i| {
            let start = i * 139 % 13000;
            (start, start + 1 + i * 37 % 300)
        })
        .collect();
    for (attack, release) in [(0, 0), (0, 193), (331, 0), (331, 193), (20000, 20000)] {
        let e = envelope(&windows, attack, release);
        for t in 0..20000 {
            let expected = windows
                .iter()
                .map(|&(start, end)| {
                    let weight = if t < start {
                        if attack == 0 {
                            0.0
                        } else {
                            (1.0 - (start - t) as f64 / attack as f64).max(0.0)
                        }
                    } else if t < end {
                        1.0
                    } else if release == 0 {
                        0.0
                    } else {
                        (1.0 - (t - end) as f64 / release as f64).max(0.0)
                    };
                    1.0 - 0.9 * weight
                })
                .fold(1.0, f64::min);
            near(e.gain_at(TimeUs(t)), expected);
        }
    }
}

#[test]
fn rejects_invalid_inputs_and_handles_full_timeline_without_overflow() {
    let new = |w, db, a, r| DuckingEnvelope::new(w, db, TimeUs(a), TimeUs(r));
    for db in [f64::NAN, f64::INFINITY, -60.1, 0.1] {
        assert!(new(vec![], db, 0, 0).is_err());
    }
    for w in [
        vec![(TimeUs(1), TimeUs(1))],
        vec![(TimeUs(2), TimeUs(1))],
        vec![(TimeUs(0), TimeUs(MAX_TIME_US + 1))],
    ] {
        assert!(new(w, -20.0, 0, 0).is_err());
    }
    assert!(new(vec![], -20.0, MAX_TIME_US + 1, 0).is_err());
    assert!(new(vec![], -20.0, 0, u64::MAX).is_err());
    let e = envelope(&[(MAX_TIME_US - 1, MAX_TIME_US)], MAX_TIME_US, MAX_TIME_US);
    near(e.gain_at(TimeUs(MAX_TIME_US)), 0.1);
    near(e.gain_at(TimeUs(u64::MAX)), 1.0);
}
