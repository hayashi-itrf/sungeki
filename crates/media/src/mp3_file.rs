//! File-backed canonical MP3 decode. Not yet connected to Project APIs.
use crate::{Mp3DecodeError as Error, ProcessSpec, StopReason, WavError, WavInfo};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// Owns only a newly created private directory; dropping it removes its files.
#[derive(Debug)]
pub struct DecodedMp3File {
    directory: PathBuf,
    info: WavInfo,
}
impl DecodedMp3File {
    pub fn path(&self) -> PathBuf {
        self.directory.join("decoded.wav")
    }
    pub fn info(&self) -> &WavInfo {
        &self.info
    }
}
impl Drop for DecodedMp3File {
    fn drop(&mut self) {
        for name in ["source.mp3", "decoded.wav"] {
            let _ = fs::remove_file(self.directory.join(name));
        }
        let _ = fs::remove_dir(&self.directory);
    }
}
fn check(end: Instant, cancel: &AtomicBool) -> Result<Duration, Error> {
    if cancel.load(Ordering::Acquire) {
        return Err(Error::Cancelled);
    }
    end.checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or(Error::Stopped(StopReason::Deadline))
}

/// At most two hours of 48kHz stereo float32, checked without whole-output Vecs.
/// The caller supplies captured original bytes and an external SSD work root.
pub fn decode_mp3_file(
    bytes: &[u8],
    ffmpeg: &Path,
    root: &Path,
    timeout: Duration,
    max_frames: u64,
    cancel: &AtomicBool,
) -> Result<DecodedMp3File, Error> {
    if bytes.is_empty() || bytes.len() > crate::MAX_WAV_BYTES {
        return Err(Error::InputSize);
    }
    if !ffmpeg.is_absolute()
        || !root.is_absolute()
        || !fs::symlink_metadata(root)?.is_dir()
        || timeout.is_zero()
        || timeout > Duration::from_secs(3600)
        || max_frames == 0
        || max_frames > 48000 * 7200
    {
        return Err(Error::Configuration);
    }
    let end = Instant::now() + timeout;
    check(end, cancel)?;
    let max_bytes = max_frames * 8;
    crate::ensure_available_space(root, max_bytes + bytes.len() as u64 + 65536)?;
    let directory = root.join(format!("mp3-file-{}", uuid::Uuid::new_v4()));
    fs::DirBuilder::new().mode(0o700).create(&directory)?;
    let mut result = DecodedMp3File {
        directory,
        info: WavInfo {
            sample_rate: 48000,
            channels: 2,
            bits_per_sample: 32,
            is_float: true,
            frames: 0,
            duration_us: 0,
            content_hash: String::new(),
        },
    };
    let mut original = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(result.directory.join("source.mp3"))?;
    for chunk in bytes.chunks(65536) {
        check(end, cancel)?;
        original.write_all(chunk)?;
    }
    original.sync_all()?;
    drop(original);
    let mut wav = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(result.path())?;
    wav.write_all(&[0; 44])?;
    let args = [
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
    .map(Into::into);
    let output = crate::run_process_to_writer(
        &ProcessSpec {
            program: ffmpeg,
            args: &args,
            directory: &result.directory,
            timeout: check(end, cancel)?,
            max_output_bytes: 65536,
        },
        cancel,
        &mut wav,
        max_bytes as usize,
    )?;
    check(end, cancel)?;
    if let Some(reason) = output.stopped {
        return Err(Error::Stopped(reason));
    }
    if !output.succeeded() {
        return Err(Error::Decode(
            String::from_utf8_lossy(&output.stderr).into(),
        ));
    }
    let size = wav
        .metadata()?
        .len()
        .checked_sub(44)
        .ok_or(Error::Wav(WavError::Container))?;
    if size == 0 || size % 8 != 0 || size > max_bytes {
        return Err(Error::Wav(WavError::Container));
    }
    wav.seek(SeekFrom::Start(0))?;
    wav.write_all(&crate::mp3_decode::pcm_header(size as u32))?;
    wav.sync_all()?;
    // Validate every float and hash exactly the same canonical bytes as decode_mp3.
    wav.seek(SeekFrom::Start(44))?;
    let mut buffer = [0u8; 65536];
    let mut remaining = size;
    while remaining > 0 {
        check(end, cancel)?;
        let n = remaining.min(buffer.len() as u64) as usize;
        wav.read_exact(&mut buffer[..n])?;
        if buffer[..n]
            .chunks_exact(4)
            .any(|b| !f32::from_le_bytes(b.try_into().unwrap()).is_finite())
        {
            return Err(Error::Wav(WavError::NonFiniteSample));
        }
        remaining -= n as u64;
    }
    wav.seek(SeekFrom::Start(0))?;
    let mut digest = Sha256::new();
    loop {
        check(end, cancel)?;
        let n = wav.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    result.info.frames = size / 8;
    result.info.duration_us = (result.info.frames * 1_000_000).div_ceil(48000);
    result.info.content_hash = format!("{:x}", digest.finalize());
    drop(wav);
    fs::remove_file(result.directory.join("source.mp3"))?;
    Ok(result)
}
