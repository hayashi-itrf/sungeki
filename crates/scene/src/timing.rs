use sungeki_project::{FrameRate, MAX_TIME_US, TimeUs};

/// First frame whose integer microsecond timestamp is >= time.
/// Ceil division is required: rounding would expose a clip before its start.
pub fn first_frame_at_or_after(fps: &FrameRate, time: TimeUs) -> Option<u64> {
    if fps.numerator == 0 || fps.denominator == 0 || time.0 > MAX_TIME_US {
        return None;
    }
    let divisor = 1_000_000_u128 * fps.denominator as u128;
    let value = (time.0 as u128 * fps.numerator as u128).div_ceil(divisor);
    u64::try_from(value).ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameSpan {
    /// Half-open frame interval. The final frame may extend past project duration.
    pub first: u64,
    pub end: u64,
}
