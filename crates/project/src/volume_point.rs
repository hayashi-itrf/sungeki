use crate::{TimeUs, ValidationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Clip-relative amplitude multiplier. Interpolate linearly; hold endpoint values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VolumePoint {
    pub at_us: TimeUs,
    pub level: f64,
}

pub(crate) fn validate_points(
    points: &[VolumePoint],
    duration: TimeUs,
) -> Result<(), ValidationError> {
    if points.len() > 64
        || points
            .iter()
            .any(|p| p.at_us > duration || !p.level.is_finite() || !(0.0..=1.0).contains(&p.level))
        || points.windows(2).any(|p| p[0].at_us >= p[1].at_us)
    {
        return Err(ValidationError::new(
            "音量ポイントは64個以内、時刻は昇順かつクリップ内、音量は0〜1で指定してください".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn point_bounds_and_order_are_strict() {
        let duration = TimeUs(1_000_000);
        let valid = vec![
            VolumePoint {
                at_us: TimeUs(0),
                level: 0.0,
            },
            VolumePoint {
                at_us: duration,
                level: 1.0,
            },
        ];
        validate_points(&valid, duration).unwrap();
        for points in [
            vec![valid[0].clone(), valid[0].clone()],
            vec![valid[1].clone(), valid[0].clone()],
            vec![VolumePoint {
                at_us: TimeUs(1_000_001),
                level: 0.5,
            }],
            vec![VolumePoint {
                at_us: TimeUs(0),
                level: f64::NAN,
            }],
            vec![VolumePoint {
                at_us: TimeUs(0),
                level: -0.1,
            }],
            vec![VolumePoint {
                at_us: TimeUs(0),
                level: 1.1,
            }],
        ] {
            assert!(validate_points(&points, duration).is_err());
        }
        let mut points: Vec<_> = (0..64)
            .map(|i| VolumePoint {
                at_us: TimeUs(i),
                level: 0.5,
            })
            .collect();
        validate_points(&points, duration).unwrap();
        points.push(VolumePoint {
            at_us: TimeUs(64),
            level: 0.5,
        });
        assert!(validate_points(&points, duration).is_err());
    }
}
