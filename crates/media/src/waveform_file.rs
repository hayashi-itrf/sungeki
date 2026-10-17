//! Scan a private decoded MP3 without retaining its PCM in memory.
use crate::{DecodedMp3File, WavError, Waveform, WaveformError};
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::{BufReader, Read},
    os::unix::fs::OpenOptionsExt,
    sync::atomic::AtomicBool,
};

struct Hashed<R> {
    inner: R,
    hash: Sha256,
    bytes: u64,
}
impl<R: Read> Read for Hashed<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hash.update(&buf[..n]);
        self.bytes += n as u64;
        Ok(n)
    }
}
fn decode(e: impl std::fmt::Display) -> WaveformError {
    WavError::Decode(e.to_string()).into()
}

/// Shares bucket/RMS evaluation with the memory path. Validates the bytes
/// actually consumed against the decoder's identity before returning a result.
pub fn analyze_decoded_mp3_waveform(
    audio: &DecodedMp3File,
    base_frames: u32,
    cancel: &AtomicBool,
) -> Result<Waveform, WaveformError> {
    let info = audio.info();
    let expected_bytes = info.frames * 8 + 44;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(audio.path())
        .map_err(decode)?;
    let meta = file.metadata().map_err(decode)?;
    if !meta.is_file() || meta.len() != expected_bytes {
        return Err(WavError::Container.into());
    }
    let hashed = Hashed {
        inner: BufReader::new(file.take(expected_bytes + 1)),
        hash: Sha256::new(),
        bytes: 0,
    };
    let mut reader = hound::WavReader::new(hashed).map_err(decode)?;
    let spec = reader.spec();
    if spec.sample_rate != 48000
        || spec.channels != 2
        || spec.bits_per_sample != 32
        || spec.sample_format != hound::SampleFormat::Float
        || u64::from(reader.duration()) != info.frames
    {
        return Err(WavError::FormatMismatch.into());
    }
    let waveform = crate::waveform::analyze_samples(
        info,
        reader.samples::<f32>().map(|s| s.map(f64::from)),
        base_frames,
        cancel,
    )?;
    let mut input = reader.into_inner();
    let mut extra = [0u8; 1];
    if input.read(&mut extra).map_err(decode)? != 0
        || input.bytes != expected_bytes
        || format!("{:x}", input.hash.finalize()) != info.content_hash
    {
        return Err(WavError::Container.into());
    }
    Ok(waveform)
}
