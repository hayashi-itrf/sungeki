use sungeki_project::VolumePoint;

// A balanced decision tree bounds expression depth for all 64 supported points.
fn expression(points: &[VolumePoint], lo: usize, hi: usize) -> String {
    if lo < hi {
        let mid = (lo + hi) / 2;
        return format!(
            "if(lt(n*1000000/48000,{}),{},{})",
            points[mid].at_us.0,
            expression(points, lo, mid),
            expression(points, mid + 1, hi)
        );
    }
    if lo == 0 {
        return format!("{:.17}", points[0].level);
    }
    if lo == points.len() {
        return format!("{:.17}", points[lo - 1].level);
    }
    let a = &points[lo - 1];
    let b = &points[lo];
    format!(
        "({:.17}+({:.17})*(n*1000000/48000-{})/{})",
        a.level,
        b.level - a.level,
        a.at_us.0,
        b.at_us.0 - a.at_us.0
    )
}

pub(crate) fn filter(points: &[VolumePoint]) -> String {
    if points.is_empty() {
        return String::new();
    }
    let gain = expression(points, 0, points.len());
    format!("aeval=exprs='val(0)*{gain}|val(1)*{gain}',")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::atomic::AtomicBool, time::Duration};
    use sungeki_project::TimeUs;
    #[test]
    #[ignore = "requires VVM_TEST_FFMPEG; every-sample envelope verification"]
    fn real_sixty_four_points_interpolate_per_sample_and_hold_ends() {
        let binary = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
        let points: Vec<_> = (0..64)
            .map(|i| VolumePoint {
                at_us: TimeUs(100_001 + i * 10_001),
                level: (i % 3) as f64 / 2.0,
            })
            .collect();
        let graph = filter(&points);
        let args: Vec<_> = [
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=0.5|-0.25:s=48000:d=1",
            "-af",
            graph.trim_end_matches(','),
            "-f",
            "f64le",
            "-c:a",
            "pcm_f64le",
            "pipe:1",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let output = sungeki_media::run_process(
            &sungeki_media::ProcessSpec {
                program: &binary,
                args: &args,
                directory: &std::env::temp_dir(),
                timeout: Duration::from_secs(10),
                max_output_bytes: 1024 * 1024,
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(
            output.succeeded(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), 48000 * 2 * 8);
        for (i, bytes) in output.stdout.chunks_exact(8).enumerate() {
            let us = (i / 2) as f64 * 1_000_000.0 / 48000.0;
            let gain = if us < points[0].at_us.0 as f64 {
                points[0].level
            } else if let Some(pair) = points.windows(2).find(|p| us < p[1].at_us.0 as f64) {
                pair[0].level
                    + (pair[1].level - pair[0].level) * (us - pair[0].at_us.0 as f64)
                        / (pair[1].at_us.0 - pair[0].at_us.0) as f64
            } else {
                points.last().unwrap().level
            };
            let source = if i % 2 == 0 { 0.5 } else { -0.25 };
            let actual = f64::from_le_bytes(bytes.try_into().unwrap());
            assert!((actual - source * gain).abs() < 1e-12, "sample {i}");
        }
    }
}
