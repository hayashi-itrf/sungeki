//! Reuse the audio compositor's sample selection, without pretending that a
//! potentially multi-GiB decoded video track is a <=64MiB Project WAV asset.
use crate::{RenderError, plan::error};
use sungeki_media::WavInfo;
use sungeki_project::{TimeUs, VideoPlayback, VideoProbe};
pub struct VideoAudioSelection {
    first_sample: u64,
    end_sample: u64,
    output_samples: u64,
    delay_samples: u64,
    gain_db: f64,
    looped: bool,
}
impl VideoAudioSelection {
    pub fn new(
        playback: &VideoPlayback,
        probe: &VideoProbe,
        wav: &WavInfo,
        start: TimeUs,
        duration: TimeUs,
    ) -> Result<Self, RenderError> {
        let result = Self::from_probe(playback, probe, start, duration)?;
        let expected = Self::frames(probe);
        if wav.sample_rate != 48000
            || wav.channels != 2
            || wav.bits_per_sample != 32
            || !wav.is_float
            || wav.frames != expected
            || wav.duration_us != (expected * 1_000_000).div_ceil(48000)
        {
            return Err(error("動画の整列済み音声の時計が一致しません"));
        }
        Ok(result)
    }
    fn frames(probe: &VideoProbe) -> u64 {
        let clock = &probe.metadata.video.clock;
        (u128::from(clock.duration_ts.unwrap()) * u128::from(clock.time_base.numerator) * 48000)
            .div_ceil(u128::from(clock.time_base.denominator)) as u64
    }
    pub(crate) fn from_probe(
        playback: &VideoPlayback,
        probe: &VideoProbe,
        start: TimeUs,
        duration: TimeUs,
    ) -> Result<Self, RenderError> {
        playback.validate(probe, duration).map_err(error)?;
        if playback.audio_stream.is_none() {
            return Err(error("動画の元音声が選択されていません"));
        }
        let end = start
            .0
            .checked_add(duration.0)
            .filter(|v| *v <= sungeki_project::MAX_TIME_US)
            .ok_or_else(|| error("動画音声の終了時刻が不正です"))?;
        let (first_sample, end_sample) = crate::audio_selection::range(
            playback.source_start_us,
            playback.source_duration_us,
            48000,
            Self::frames(probe),
        );
        let at = |us: u64| (us * 48000 + 500000) / 1_000_000;
        let output_samples = at(end) - at(start.0);
        if first_sample >= end_sample || output_samples == 0 {
            return Err(error("動画音声の使用区間にサンプルがありません"));
        }
        if playback.looped && (end_sample - first_sample) * 16 > 256 * 1024 * 1024 {
            return Err(error("動画音声ループが256MiBを超えています"));
        }
        Ok(Self {
            first_sample,
            end_sample,
            output_samples,
            delay_samples: at(start.0),
            gain_db: playback.gain_db,
            looped: playback.looped,
        })
    }
    pub(crate) fn loop_bytes(&self) -> u64 {
        if self.looped {
            (self.end_sample - self.first_sample) * 16
        } else {
            0
        }
    }
    /// One aligned stereo input -> clip-length samples, placed on project time.
    /// No inferred ducking or fades. Mixing/mastering remain the caller's job.
    pub fn filter(&self) -> String {
        format!(
            "{}volume={:.15}:precision=double,atrim=end_sample={},adelay={}S:all=1",
            crate::audio_selection::trim_loop(self.first_sample, self.end_sample, self.looped),
            10f64.powf(self.gain_db / 20.0),
            self.output_samples,
            self.delay_samples
        )
    }
}
