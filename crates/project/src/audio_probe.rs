use crate::{MAX_TIME_US, TimeUs, ValidationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Compressed source identity is distinct from its canonical decoded waveform.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompressedAudioEncoding {
    Mp3 {
        decode_version: u32,
        decoded_sha256: String,
    },
}

/// Source hash/byte length and measured decoded sample clock.
/// `encoding: None` describes a WAV original; compressed inputs must declare it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AudioProbe {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub encoding: Option<CompressedAudioEncoding>,
    pub sha256: String,
    pub duration_us: TimeUs,
    pub sample_rate: u32,
    pub channels: u16,
    #[ts(type = "number")]
    pub frames: u64,
    #[ts(type = "number")]
    pub byte_length: u64,
}
impl AudioProbe {
    fn valid_encoding(&self) -> bool {
        match &self.encoding {
            None => self
                .frames
                .checked_mul(u64::from(self.channels))
                .is_some_and(|v| v <= self.byte_length.saturating_sub(44)),
            Some(CompressedAudioEncoding::Mp3 {
                decode_version,
                decoded_sha256,
            }) => {
                *decode_version == 1
                    && self.sample_rate == 48000
                    && self.channels == 2
                    && decoded_sha256.len() == 64
                    && decoded_sha256
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                    && self.frames <= 48000 * 7200
            }
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        let duration = self
            .frames
            .checked_mul(1_000_000)
            .and_then(|v| v.checked_add(u64::from(self.sample_rate).saturating_sub(1)))
            .and_then(|v| v.checked_div(u64::from(self.sample_rate)));
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || !(1..=2).contains(&self.channels)
            || self.frames == 0
            || self.duration_us.0 > MAX_TIME_US
            || duration != Some(self.duration_us.0)
            || !(44..=64 * 1024 * 1024).contains(&self.byte_length)
            || !self.valid_encoding()
        {
            return Err(ValidationError::new(
                "音声原本のメタデータが不正です".into(),
            ));
        }
        Ok(())
    }
}
