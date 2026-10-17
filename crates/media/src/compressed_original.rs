//! One decode identity shared by import, waveform, measurement and render callers.
use crate::{Mp3DecodeError, VerifiedWav, WaveformCacheError};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::atomic::AtomicBool, time::Duration};
use sungeki_project::{AudioProbe, CompressedAudioEncoding, TimeUs};

pub struct AudioDecodeRuntime<'a> {
    pub ffmpeg: &'a Path,
    pub root: &'a Path,
    pub timeout: Duration,
}
#[derive(Debug)]
pub enum AudioDecodeError {
    Source(WaveformCacheError),
    Mp3(Mp3DecodeError),
    Wav(crate::WavError),
    Mismatch,
    MissingRuntime,
}
impl std::fmt::Display for AudioDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(e) => e.fmt(f),
            Self::Mp3(e) => e.fmt(f),
            Self::Wav(e) => e.fmt(f),
            Self::Mismatch => f.write_str("復号した音声が登録時の内容・尺と一致しません"),
            Self::MissingRuntime => f.write_str("MP3復号用のFFmpeg実行環境が必要です"),
        }
    }
}
impl std::error::Error for AudioDecodeError {}

pub fn probe_mp3(
    path: &Path,
    runtime: &AudioDecodeRuntime<'_>,
    cancel: &AtomicBool,
) -> Result<AudioProbe, AudioDecodeError> {
    let bytes =
        crate::audio_original::read(path, None, cancel).map_err(AudioDecodeError::Source)?;
    let wav = crate::decode_mp3_file(
        &bytes,
        runtime.ffmpeg,
        runtime.root,
        runtime.timeout,
        48000 * 7200,
        cancel,
    )
    .map_err(AudioDecodeError::Mp3)?;
    let info = wav.info();
    let probe = AudioProbe {
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        byte_length: bytes.len() as u64,
        encoding: Some(CompressedAudioEncoding::Mp3 {
            decode_version: 1,
            decoded_sha256: info.content_hash.clone(),
        }),
        sample_rate: info.sample_rate,
        channels: info.channels,
        frames: info.frames,
        duration_us: TimeUs(info.duration_us),
    };
    probe.validate().map_err(|_| AudioDecodeError::Mismatch)?;
    Ok(probe)
}

/// Every call validates the original. A stored decoded hash never substitutes
/// for reading/capturing the source, and mismatched decoder output fails closed.
pub fn decode_audio_original(
    path: &Path,
    probe: &AudioProbe,
    runtime: Option<&AudioDecodeRuntime<'_>>,
    cancel: &AtomicBool,
) -> Result<VerifiedWav, AudioDecodeError> {
    let bytes =
        crate::audio_original::capture(path, probe, cancel).map_err(AudioDecodeError::Source)?;
    decode_captured(bytes, probe, runtime, cancel)
}

pub(crate) fn decode_captured(
    bytes: Vec<u8>,
    probe: &AudioProbe,
    runtime: Option<&AudioDecodeRuntime<'_>>,
    cancel: &AtomicBool,
) -> Result<VerifiedWav, AudioDecodeError> {
    let wav = match &probe.encoding {
        None => crate::verify_wav(
            bytes,
            crate::ExpectedAudio {
                sample_rate: probe.sample_rate,
                channels: probe.channels,
            },
        )
        .map_err(AudioDecodeError::Wav)?,
        Some(CompressedAudioEncoding::Mp3 { decoded_sha256, .. }) => {
            let runtime = runtime.ok_or(AudioDecodeError::MissingRuntime)?;
            let wav = crate::decode_mp3(
                &bytes,
                runtime.ffmpeg,
                runtime.root,
                runtime.timeout,
                cancel,
            )
            .map_err(AudioDecodeError::Mp3)?;
            if wav.info().content_hash != *decoded_sha256 {
                return Err(AudioDecodeError::Mismatch);
            }
            wav
        }
    };
    verify_decoded(wav.info(), probe)?;
    Ok(wav)
}

pub(crate) fn waveform_error(cause: AudioDecodeError) -> WaveformCacheError {
    match cause {
        AudioDecodeError::Source(e) => e,
        AudioDecodeError::Mp3(e) => WaveformCacheError::Decode(e),
        AudioDecodeError::Wav(e) => WaveformCacheError::Wav(e),
        AudioDecodeError::Mismatch => WaveformCacheError::SourceChanged,
        AudioDecodeError::MissingRuntime => WaveformCacheError::MissingRuntime,
    }
}

/// Capture the compressed original, then validate a file-backed decode against
/// the saved identity. The returned value owns the temporary decoded file.
pub fn decode_mp3_original_file(
    path: &Path,
    probe: &AudioProbe,
    runtime: &AudioDecodeRuntime<'_>,
    cancel: &AtomicBool,
) -> Result<crate::DecodedMp3File, AudioDecodeError> {
    if probe.encoding.is_none() {
        return Err(AudioDecodeError::Mismatch);
    }
    let bytes =
        crate::audio_original::capture(path, probe, cancel).map_err(AudioDecodeError::Source)?;
    decode_captured_file(bytes, probe, runtime, cancel)
}

pub(crate) fn decode_captured_file(
    bytes: Vec<u8>,
    probe: &AudioProbe,
    runtime: &AudioDecodeRuntime<'_>,
    cancel: &AtomicBool,
) -> Result<crate::DecodedMp3File, AudioDecodeError> {
    let audio = crate::decode_mp3_file(
        &bytes,
        runtime.ffmpeg,
        runtime.root,
        runtime.timeout,
        probe.frames,
        cancel,
    )
    .map_err(|e| match e {
        Mp3DecodeError::Stopped(crate::StopReason::OutputLimit) => AudioDecodeError::Mismatch,
        other => AudioDecodeError::Mp3(other),
    })?;
    verify_decoded(audio.info(), probe)?;
    Ok(audio)
}

fn verify_decoded(info: &crate::WavInfo, probe: &AudioProbe) -> Result<(), AudioDecodeError> {
    if info.frames != probe.frames
        || info.duration_us != probe.duration_us.0
        || info.sample_rate != probe.sample_rate
        || info.channels != probe.channels
        || matches!(&probe.encoding, Some(CompressedAudioEncoding::Mp3 { decoded_sha256, .. })
            if *decoded_sha256 != info.content_hash)
    {
        return Err(AudioDecodeError::Mismatch);
    }
    Ok(())
}
