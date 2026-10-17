use hound::SampleFormat;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpectedAudio {
    pub sample_rate: u32,
    pub channels: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WavInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub is_float: bool,
    /// Sample frames (independent of the number of channels).
    pub frames: u64,
    /// Rounded UP to microseconds so a clip cannot truncate its last sample.
    pub duration_us: u64,
    pub content_hash: String,
}
#[derive(Debug)]
pub struct VerifiedWav {
    bytes: Vec<u8>,
    info: WavInfo,
}
impl VerifiedWav {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn info(&self) -> &WavInfo {
        &self.info
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WavError {
    TooLarge,
    Container,
    Decode(String),
    Empty,
    FormatMismatch,
    UnsupportedFormat,
    NonFiniteSample,
}
impl std::fmt::Display for WavError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => f.write_str("WAVは64MiBまでです"),
            Self::Container => f.write_str("WAVの構造・長さが不正です"),
            Self::Decode(message) => write!(f, "WAVを読み取れません: {message}"),
            Self::Empty => f.write_str("音声サンプルがありません"),
            Self::FormatMismatch => {
                f.write_str("音声のサンプリングレートまたはチャンネル数が設定と一致しません")
            }
            Self::UnsupportedFormat => f.write_str("対応していないWAV音声形式です"),
            Self::NonFiniteSample => f.write_str("音声に非有限のサンプル値が含まれています"),
        }
    }
}
impl std::error::Error for WavError {}
fn decode(error: hound::Error) -> WavError {
    WavError::Decode(error.to_string())
}

/// Bounded synchronous validation: run on a media/blocking worker, not while
/// holding the editor lock. Validity is not a loudness or pronunciation review.
pub fn verify_wav(bytes: Vec<u8>, expected: ExpectedAudio) -> Result<VerifiedWav, WavError> {
    let mut reader = crate::wav_container::reader(&bytes)?;
    let spec = reader.spec();
    if spec.sample_rate == 0 || !(1..=2).contains(&spec.channels) {
        return Err(WavError::UnsupportedFormat);
    }
    if spec.sample_rate != expected.sample_rate || spec.channels != expected.channels {
        return Err(WavError::FormatMismatch);
    }
    let mut count = 0u64;
    match spec.sample_format {
        SampleFormat::Int => {
            for sample in reader.samples::<i32>() {
                sample.map_err(decode)?;
                count += 1;
            }
        }
        SampleFormat::Float => {
            for sample in reader.samples::<f32>() {
                if !sample.map_err(decode)?.is_finite() {
                    return Err(WavError::NonFiniteSample);
                }
                count += 1;
            }
        }
    }
    if count == 0 {
        return Err(WavError::Empty);
    }
    if count != u64::from(reader.len()) || count % u64::from(spec.channels) != 0 {
        return Err(WavError::Container);
    }
    let frames = count / u64::from(spec.channels);
    let info = WavInfo {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        bits_per_sample: spec.bits_per_sample,
        is_float: spec.sample_format == SampleFormat::Float,
        frames,
        duration_us: (frames * 1_000_000).div_ceil(u64::from(spec.sample_rate)),
        content_hash: format!("{:x}", Sha256::digest(&bytes)),
    };
    drop(reader);
    Ok(VerifiedWav { bytes, info })
}

/// Inspect an imported WAV without trusting caller-supplied rate/channel metadata.
pub fn inspect_wav(bytes: Vec<u8>) -> Result<VerifiedWav, WavError> {
    let spec = crate::wav_container::reader(&bytes)?.spec();
    verify_wav(
        bytes,
        ExpectedAudio {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
        },
    )
}
