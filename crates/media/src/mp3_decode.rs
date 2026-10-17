//! Decode captured MP3 bytes into canonical PCM; no Project edits or publication.
use crate::{ExpectedAudio, ProcessSpec, StopReason, VerifiedWav, run_process, verify_wav};
use std::{
    ffi::OsString,
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[derive(Debug)]
pub enum Mp3DecodeError {
    Configuration,
    InputSize,
    Cancelled,
    Stopped(StopReason),
    Decode(String),
    Io(std::io::Error),
    Wav(crate::WavError),
}
impl std::fmt::Display for Mp3DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configuration => f.write_str("MP3復号環境が不正です"),
            Self::InputSize => f.write_str("MP3原本は空でなく64MiB以下である必要があります"),
            Self::Cancelled => f.write_str("MP3復号を取り消しました"),
            Self::Stopped(StopReason::OutputLimit) => {
                f.write_str("復号後の音声が許容サイズの上限を超えます")
            }
            Self::Stopped(StopReason::Deadline) => {
                f.write_str("MP3の読み込みが制限時間内に完了しませんでした")
            }
            Self::Stopped(reason) => write!(f, "MP3復号を中断しました: {reason:?}"),
            Self::Decode(message) => write!(f, "MP3を復号できません: {message}"),
            Self::Io(error) => error.fmt(f),
            Self::Wav(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for Mp3DecodeError {}
impl From<std::io::Error> for Mp3DecodeError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
struct Work(PathBuf);
impl Drop for Work {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join("source.mp3"));
        let _ = fs::remove_dir(&self.0);
    }
}

/// The caller supplies owned, hash-verified originals and an SSD work root.
/// Decode all samples, never trim to fit the limit. Float PCM retains MP3
/// overshoots for later gain/mix processing. Rate/channels match the mix engine.
/// The current VerifiedWav limit also bounds decoded output (about 175 seconds).
/// Longer sources fail explicitly; streaming long-source support is still needed.
pub fn decode_mp3(
    bytes: &[u8],
    ffmpeg: &Path,
    root: &Path,
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<VerifiedWav, Mp3DecodeError> {
    if cancel.load(Ordering::Acquire) {
        return Err(Mp3DecodeError::Cancelled);
    }
    if bytes.is_empty() || bytes.len() > crate::MAX_WAV_BYTES {
        return Err(Mp3DecodeError::InputSize);
    }
    if !ffmpeg.is_absolute()
        || !root.is_absolute()
        || !fs::symlink_metadata(root)?.is_dir()
        || timeout.is_zero()
        || timeout > Duration::from_secs(3600)
    {
        return Err(Mp3DecodeError::Configuration);
    }
    let directory = root.join(format!("mp3-decode-{}", uuid::Uuid::new_v4()));
    fs::DirBuilder::new().mode(0o700).create(&directory)?;
    let work = Work(directory);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(work.0.join("source.mp3"))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    let args: Vec<OsString> = [
        "-nostdin",
        "-v",
        "error",
        "-xerror",
        "-err_detect",
        "explode",
        "-f",
        "mp3",
        "-i",
        "source.mp3",
        "-map",
        "0:a:0",
        "-vn",
        "-sn",
        "-dn",
        "-ar",
        "48000",
        "-ac",
        "2",
        "-c:a",
        "pcm_f32le",
        "-f",
        "f32le",
        "pipe:1",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    let result = run_process(
        &ProcessSpec {
            program: ffmpeg,
            args: &args,
            directory: &work.0,
            timeout,
            max_output_bytes: crate::MAX_WAV_BYTES - 44,
        },
        cancel,
    )
    .map_err(|error| {
        if cancel.load(Ordering::Acquire) {
            Mp3DecodeError::Cancelled
        } else {
            Mp3DecodeError::Io(error)
        }
    })?;
    if cancel.load(Ordering::Acquire) {
        return Err(Mp3DecodeError::Cancelled);
    }
    if let Some(reason) = result.stopped {
        return Err(Mp3DecodeError::Stopped(reason));
    }
    if !result.succeeded() {
        return Err(Mp3DecodeError::Decode(
            String::from_utf8_lossy(&result.stderr).into_owned(),
        ));
    }
    pcm_wav(result.stdout).map_err(Mp3DecodeError::Wav)
}

fn pcm_wav(pcm: Vec<u8>) -> Result<VerifiedWav, crate::WavError> {
    if pcm.is_empty() || pcm.len() % 8 != 0 {
        return Err(crate::WavError::Container);
    }
    let mut bytes = pcm_header(pcm.len() as u32);
    bytes.extend_from_slice(&pcm);
    verify_wav(
        bytes,
        ExpectedAudio {
            sample_rate: 48000,
            channels: 2,
        },
    )
}

pub(crate) fn pcm_header(size: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(44);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(size + 36).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt \x10\0\0\0\x03\0\x02\0");
    bytes.extend_from_slice(&48000u32.to_le_bytes());
    bytes.extend_from_slice(&(48000u32 * 8).to_le_bytes());
    bytes.extend_from_slice(b"\x08\0\x20\0data");
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes
}
