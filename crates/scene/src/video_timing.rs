//! Pure timeline-to-source mapping. Stream start_pts is applied by the decoder,
//! once, after this mapping; it is never added to project time.
use sungeki_project::{
    Clip, ClipContent, MAX_TIME_US, TimeUs, ValidationError, VideoPlayback, VideoProbe,
};
#[derive(Debug, Clone)]
pub struct VideoClipTiming {
    start: TimeUs,
    end: TimeUs,
    playback: VideoPlayback,
}
impl VideoClipTiming {
    pub fn new(clip: &Clip, probe: &VideoProbe) -> Result<Self, ValidationError> {
        let ClipContent::Video { playback, .. } = &clip.content else {
            return Err(ValidationError::new("動画クリップではありません".into()));
        };
        playback.validate(probe, clip.duration_us)?;
        let end = clip
            .start_us
            .0
            .checked_add(clip.duration_us.0)
            .filter(|v| *v <= MAX_TIME_US)
            .ok_or_else(|| ValidationError::new("動画クリップの終了時刻が不正です".into()))?;
        Ok(Self {
            start: clip.start_us,
            end: TimeUs(end),
            playback: playback.clone(),
        })
    }
    /// Half-open clip interval. A loop restarts exactly at its source boundary.
    pub fn source_time(&self, time: TimeUs) -> Option<TimeUs> {
        if time < self.start || time >= self.end {
            return None;
        }
        let elapsed = time.0 - self.start.0;
        let elapsed = if self.playback.looped {
            elapsed % self.playback.source_duration_us.0
        } else {
            elapsed
        };
        Some(TimeUs(self.playback.source_start_us.0 + elapsed))
    }
}
