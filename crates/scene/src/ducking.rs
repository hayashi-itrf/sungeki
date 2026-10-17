//! Timeline-driven ducking, independent of GUI, codec and sample rate.
use sungeki_project::{Ducking, MAX_TIME_US, TimeUs, ValidationError};

/// Multiplicative linear-amplitude envelope. Speech windows are half-open.
/// Attack precedes speech; release follows it. Overlapping windows never stack
/// attenuation. Memory is proportional to windows, not timeline duration.
#[derive(Debug, Clone)]
pub struct DuckingEnvelope {
    windows: Vec<(u64, u64)>,
    floor: f64,
    attack: u64,
    release: u64,
}

impl DuckingEnvelope {
    pub fn windows(&self) -> impl Iterator<Item = (TimeUs, TimeUs)> + '_ {
        self.windows
            .iter()
            .map(|&(start, end)| (TimeUs(start), TimeUs(end)))
    }
    pub fn new(
        windows: impl IntoIterator<Item = (TimeUs, TimeUs)>,
        attenuation_db: f64,
        attack: TimeUs,
        release: TimeUs,
    ) -> Result<Self, ValidationError> {
        Ducking {
            attenuation_db,
            attack_us: attack,
            release_us: release,
        }
        .validate()?;
        let mut sorted = Vec::new();
        for (start, end) in windows {
            if start >= end || end.0 > MAX_TIME_US {
                return Err(ValidationError::new(
                    "ダッキングの発話区間が不正です".into(),
                ));
            }
            sorted.push((start.0, end.0));
        }
        sorted.sort_unstable();
        let mut merged: Vec<(u64, u64)> = Vec::with_capacity(sorted.len());
        for (start, end) in sorted {
            if let Some(last) = merged.last_mut().filter(|last| start <= last.1) {
                last.1 = last.1.max(end);
            } else {
                merged.push((start, end));
            }
        }
        Ok(Self {
            windows: merged,
            floor: 10f64.powf(attenuation_db / 20.0),
            attack: attack.0,
            release: release.0,
        })
    }

    /// Evaluate using the absolute timeline time, including for a trimmed BGM.
    /// At zero-duration attack/release boundaries changes are instantaneous.
    /// A speech starting at zero is already attenuated at the first sample.
    pub fn gain_at(&self, time: TimeUs) -> f64 {
        let t = time.0;
        let next = self.windows.partition_point(|window| window.0 <= t);
        let mut weight: f64 = 0.0;
        if next > 0 {
            let end = self.windows[next - 1].1;
            if t < end {
                return self.floor;
            }
            if self.release > 0 {
                weight = (1.0 - (t - end) as f64 / self.release as f64).max(0.0);
            }
        }
        if let Some(&(start, _)) = self.windows.get(next) {
            if self.attack > 0 {
                weight = weight.max((1.0 - (start - t) as f64 / self.attack as f64).max(0.0));
            }
        }
        1.0 + (self.floor - 1.0) * weight
    }
}
