//! Exact, one-to-one frame correspondence for a future lightweight preview.
//! This validates timing only, not pixels, orientation, audio or cache identity.
use sungeki_project::{MediaStreamClock, TimeUs};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyFrame {
    pub source_pts: i64,
    pub proxy_pts: i64,
}

#[derive(Debug)]
pub struct ProxyTimeline {
    source_clock: MediaStreamClock,
    frames: Vec<ProxyFrame>,
}

impl ProxyTimeline {
    /// Reject dropped/duplicated frames, rounded timestamps and altered duration.
    /// No average-FPS inference or nearest-frame matching is permitted.
    pub fn verify(
        source_clock: &MediaStreamClock,
        source_pts: &[i64],
        proxy_clock: &MediaStreamClock,
        proxy_pts: &[i64],
    ) -> Result<Self, &'static str> {
        validate(source_clock, source_pts)?;
        validate(proxy_clock, proxy_pts)?;
        if source_pts.len() != proxy_pts.len() {
            return Err("proxy frame count differs from original");
        }
        equivalent(
            i128::from(source_clock.duration_ts.unwrap()),
            source_clock,
            i128::from(proxy_clock.duration_ts.unwrap()),
            proxy_clock,
        )?;
        let source_origin = i128::from(source_clock.start_pts.unwrap());
        let proxy_origin = i128::from(proxy_clock.start_pts.unwrap());
        for (&source, &proxy) in source_pts.iter().zip(proxy_pts) {
            equivalent(
                i128::from(source) - source_origin,
                source_clock,
                i128::from(proxy) - proxy_origin,
                proxy_clock,
            )?;
        }
        Ok(Self {
            source_clock: source_clock.clone(),
            frames: source_pts
                .iter()
                .zip(proxy_pts)
                .map(|(&source_pts, &proxy_pts)| ProxyFrame {
                    source_pts,
                    proxy_pts,
                })
                .collect(),
        })
    }

    pub fn frames(&self) -> &[ProxyFrame] {
        &self.frames
    }

    /// Offset is relative to the ORIGINAL stream origin, as in VideoPlayback.
    /// Preserve a leading gap and hold the preceding frame through VFR gaps.
    pub fn frame_at(&self, offset: TimeUs) -> Result<Option<&ProxyFrame>, &'static str> {
        let clock = &self.source_clock;
        let numerator = i128::from(clock.time_base.numerator) * 1_000_000;
        let offset = i128::from(offset.0) * i128::from(clock.time_base.denominator);
        if offset >= i128::from(clock.duration_ts.unwrap()) * numerator {
            return Err("time outside original video");
        }
        let origin = i128::from(clock.start_pts.unwrap());
        let index = self
            .frames
            .partition_point(|frame| (i128::from(frame.source_pts) - origin) * numerator <= offset);
        Ok(index.checked_sub(1).map(|index| &self.frames[index]))
    }
}

fn validate(clock: &MediaStreamClock, pts: &[i64]) -> Result<(), &'static str> {
    let (Some(origin), Some(duration)) = (clock.start_pts, clock.duration_ts) else {
        return Err("unknown stream timing");
    };
    if clock.time_base.numerator == 0
        || clock.time_base.denominator == 0
        || duration == 0
        || pts.is_empty()
        || pts.len() > 250_000
        || pts.windows(2).any(|pair| pair[0] >= pair[1])
        || pts
            .iter()
            .any(|&pts| i128::from(pts) >= i128::from(origin) + i128::from(duration))
    {
        return Err("invalid frame timeline");
    }
    Ok(())
}

fn equivalent(
    a: i128,
    ac: &MediaStreamClock,
    b: i128,
    bc: &MediaStreamClock,
) -> Result<(), &'static str> {
    let scale = |ticks: i128, numerator: u32, denominator: u32| {
        ticks
            .checked_mul(i128::from(numerator))
            .and_then(|n| n.checked_mul(i128::from(denominator)))
            .ok_or("timestamp comparison overflow")
    };
    if scale(a, ac.time_base.numerator, bc.time_base.denominator)?
        != scale(b, bc.time_base.numerator, ac.time_base.denominator)?
    {
        return Err("proxy presentation timing differs from original");
    }
    Ok(())
}

#[cfg(test)]
#[path = "proxy_timeline_tests.rs"]
mod tests;
