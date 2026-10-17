use crate::{MAX_TIME_US, TimeUs, ValidationError, VideoProbe};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
/// Source times are offsets from the selected video stream's recorded start_pts
/// timestamp, NOT the container origin. Audio retains its own clock and offset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VideoPlayback {
    pub source_start_us: TimeUs,
    pub source_duration_us: TimeUs,
    pub looped: bool,
    /// None explicitly mutes embedded audio; never choose a stream implicitly.
    pub audio_stream: Option<u32>,
    pub gain_db: f64,
}
impl VideoPlayback {
    pub fn validate(&self, probe: &VideoProbe, duration: TimeUs) -> Result<(), ValidationError> {
        probe.validate()?;
        let invalid =
            || ValidationError::new("動画の使用区間・音声トラック・音量が不正です".into());
        let source_duration = probe
            .metadata
            .video
            .clock
            .duration_us()?
            .ok_or_else(|| ValidationError::new("尺が不明な動画は配置できません".into()))?;
        if probe.metadata.video.clock.start_pts.is_none()
            || duration.0 == 0
            || duration.0 > MAX_TIME_US
            || self.source_duration_us.0 == 0
            || self
                .source_start_us
                .0
                .checked_add(self.source_duration_us.0)
                .is_none_or(|end| end > source_duration.0)
            || (!self.looped && duration > self.source_duration_us)
            || !self.gain_db.is_finite()
            || !(-60.0..=24.0).contains(&self.gain_db)
        {
            return Err(invalid());
        }
        if let Some(index) = self.audio_stream {
            let audio = probe
                .metadata
                .audio
                .iter()
                .find(|s| s.index == index)
                .ok_or_else(invalid)?;
            if audio.clock.start_pts.is_none() {
                return Err(ValidationError::new(
                    "開始時刻が不明な動画音声は選択できません".into(),
                ));
            }
        }
        Ok(())
    }
}
