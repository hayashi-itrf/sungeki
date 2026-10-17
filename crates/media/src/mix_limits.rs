use crate::LoudnessError;
use sungeki_project::{FrameRate, TimeUs};

/// Distinguish the document's duration from whole video frames and AAC padding.
#[derive(Clone, Copy, Debug)]
pub struct MixFrameLimit {
    pub(crate) min: u64,
    pub(crate) max: u64,
}
impl MixFrameLimit {
    pub(crate) fn source() -> Self {
        Self {
            min: 1,
            max: 7200 * 48000,
        }
    }
    pub fn for_video(
        duration: TimeUs,
        fps: &FrameRate,
        decoded_aac: bool,
    ) -> Result<Self, LoudnessError> {
        if duration.0 == 0
            || duration.0 > 7_200_000_000
            || fps.numerator == 0
            || fps.denominator == 0
        {
            return Err(LoudnessError::InvalidRuntime);
        }
        let frames = (u128::from(duration.0) * u128::from(fps.numerator))
            .div_ceil(1_000_000 * u128::from(fps.denominator));
        let samples =
            (frames * u128::from(fps.denominator) * 48000).div_ceil(u128::from(fps.numerator));
        let padding = if decoded_aac { 1024 } else { 0 };
        if frames > 432000 || samples == 0 || (samples + padding) * 8 + 65536 > u128::from(u32::MAX)
        {
            return Err(LoudnessError::InvalidRuntime);
        }
        Ok(Self {
            min: (samples as u64).saturating_sub(padding as u64).max(1),
            max: (samples + padding) as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_hours_rounding_and_codec_padding_are_separate_bounds() {
        let rate = FrameRate {
            numerator: 30000,
            denominator: 1001,
        };
        let exact = MixFrameLimit::for_video(TimeUs(7_200_000_000), &rate, false).unwrap();
        assert_eq!(exact.min, 345_601_256);
        assert_eq!(exact.min, exact.max);
        let decoded = MixFrameLimit::for_video(TimeUs(7_200_000_000), &rate, true).unwrap();
        assert_eq!(decoded.max, exact.max + 1024);
        assert_eq!(decoded.min, exact.min - 1024);
        assert!(MixFrameLimit::for_video(TimeUs(7_200_000_001), &rate, true).is_err());
        assert!(
            MixFrameLimit::for_video(
                TimeUs(1),
                &FrameRate {
                    numerator: 0,
                    denominator: 1
                },
                false
            )
            .is_err()
        );
        assert!(
            MixFrameLimit::for_video(
                TimeUs(1),
                &FrameRate {
                    numerator: 1,
                    denominator: u32::MAX
                },
                false
            )
            .is_err()
        );
    }
}
