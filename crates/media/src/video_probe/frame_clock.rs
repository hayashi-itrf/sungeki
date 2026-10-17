use super::{ProbeError, ProbedVideo};
use serde::Deserialize;
use sungeki_project::TimeUs;
#[derive(Deserialize)]
struct Frames {
    frames: Vec<Frame>,
}
#[derive(Deserialize)]
struct Frame {
    pts: i64,
    width: u32,
    height: u32,
}
pub(super) struct Clock {
    pub pts: Vec<i64>,
    origin: i64,
    duration: u64,
    numerator: i128,
    denominator: i128,
}
impl Clock {
    pub fn parse(bytes: &[u8], probe: &ProbedVideo) -> Result<Self, ProbeError> {
        let input: Frames =
            serde_json::from_slice(bytes).map_err(|_| ProbeError::Invalid("missing frame PTS"))?;
        let stream = &probe.metadata.video;
        if input.frames.is_empty()
            || input.frames.len() > 250_000
            || input
                .frames
                .iter()
                .any(|f| f.width != stream.width || f.height != stream.height)
            || input.frames.windows(2).any(|f| f[0].pts >= f[1].pts)
            || stream
                .frame_count
                .is_some_and(|n| n != input.frames.len() as u64)
        {
            return Err(ProbeError::Invalid(
                "incomplete/ambiguous frame timeline or changing dimensions",
            ));
        }
        let origin = stream
            .clock
            .start_pts
            .ok_or(ProbeError::Invalid("unknown video origin"))?;
        let duration = stream
            .clock
            .duration_ts
            .ok_or(ProbeError::Invalid("unknown video duration"))?;
        if input
            .frames
            .iter()
            .any(|f| i128::from(f.pts) >= i128::from(origin) + i128::from(duration))
        {
            return Err(ProbeError::Invalid("frame PTS outside recorded duration"));
        }
        Ok(Self {
            pts: input.frames.into_iter().map(|f| f.pts).collect(),
            origin,
            duration,
            numerator: i128::from(stream.clock.time_base.numerator),
            denominator: i128::from(stream.clock.time_base.denominator),
        })
    }
    /// Hold a frame until the next presentation timestamp. Never pick a future
    /// frame at a cut; a leading gap has no frame. No average-FPS approximation.
    pub fn index(&self, offset: TimeUs) -> Result<Option<usize>, ProbeError> {
        let offset = i128::from(offset.0) * self.denominator;
        if offset >= i128::from(self.duration) * self.numerator * 1_000_000 {
            return Err(ProbeError::Invalid("time outside video"));
        }
        let n = self.pts.partition_point(|pts| {
            (i128::from(*pts) - i128::from(self.origin)) * self.numerator * 1_000_000 <= offset
        });
        Ok(n.checked_sub(1))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_pts_boundaries_negative_origin_and_leading_gap() {
        let c = Clock {
            pts: vec![-3000, 0, 6000],
            origin: -3000,
            duration: 12000,
            numerator: 1,
            denominator: 90000,
        };
        assert_eq!(c.index(TimeUs(33333)).unwrap(), Some(0));
        assert_eq!(c.index(TimeUs(33334)).unwrap(), Some(1));
        assert_eq!(c.index(TimeUs(99999)).unwrap(), Some(1));
        assert_eq!(c.index(TimeUs(100000)).unwrap(), Some(2));
        assert!(c.index(TimeUs(133334)).is_err());
        assert!(c.index(TimeUs(u64::MAX)).is_err());
        let c = Clock {
            pts: vec![10, 20],
            origin: 0,
            duration: 30,
            numerator: 1,
            denominator: 1000,
        };
        assert_eq!(c.index(TimeUs(9999)).unwrap(), None);
        assert_eq!(c.index(TimeUs(10000)).unwrap(), Some(0));
    }
    #[test]
    fn missing_duplicate_out_of_order_and_incomplete_frames_are_rejected() {
        let p: sungeki_project::Project = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/video-project.json"
        ))
        .unwrap();
        let sungeki_project::AssetProbe::Video(mut p) = p.assets[0].probe.clone() else {
            panic!()
        };
        p.metadata.video.frame_count = None;
        for frames in [
            serde_json::json!([]),
            serde_json::json!([{"pts":90000,"width":320,"height":180}]),
            serde_json::json!([{"width":320,"height":180}]),
            serde_json::json!([{"pts":0,"width":319,"height":180}]),
            serde_json::json!([{"pts":1,"width":320,"height":180},{"pts":0,"width":320,"height":180}]),
            serde_json::json!([{"pts":0,"width":320,"height":180},{"pts":0,"width":320,"height":180}]),
        ] {
            assert!(
                Clock::parse(
                    &serde_json::to_vec(&serde_json::json!({"frames":frames})).unwrap(),
                    &p
                )
                .is_err()
            );
        }
    }
}
