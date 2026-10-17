//! Bounded-memory measurement of a captured pre-master float32 stereo mix.
use crate::{
    LOUDNESS_VERSION, LoudnessAnalysis, LoudnessError, LoudnessUnavailable, LoudnessValue,
    StopReason,
    loudness::{Work, check, run_captured},
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

fn remaining(end: Instant, flag: &AtomicBool) -> Result<Duration, LoudnessError> {
    check(flag)?;
    end.checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or(LoudnessError::Stopped(StopReason::Deadline))
}

/// Measure a decoded original, without gain. The decoder owns its private file;
/// the shared file scanner checks its exact hash and frame count again.
/// The caller replaces the decoded hash with the compressed original identity.
pub fn measure_decoded_mp3_loudness(
    audio: &crate::DecodedMp3File,
    ffmpeg: &Path,
    root: &Path,
    timeout: Duration,
    flag: &AtomicBool,
) -> Result<LoudnessAnalysis, LoudnessError> {
    let info = audio.info();
    let mut analysis = measure_render_mix_loudness(
        &audio.path(),
        &info.content_hash,
        ffmpeg,
        root,
        timeout,
        flag,
        crate::MixFrameLimit {
            min: info.frames,
            max: info.frames,
        },
    )?;
    analysis.source_pre_gain = true;
    Ok(analysis)
}

/// Separate from the 64MiB original-asset API. Accept only 48kHz stereo float32
/// WAV, at most two hours. Copy, hash and validate samples incrementally before
/// FFmpeg reads the private capture. Never reports this as source-pre-gain.
pub fn measure_mix_loudness(
    source: &Path,
    expected_sha256: &str,
    ffmpeg: &Path,
    root: &Path,
    timeout: Duration,
    flag: &AtomicBool,
) -> Result<LoudnessAnalysis, LoudnessError> {
    measure_render_mix_loudness(
        source,
        expected_sha256,
        ffmpeg,
        root,
        timeout,
        flag,
        crate::MixFrameLimit::source(),
    )
}

/// Render supplies a document-derived frame window; only AAC may include padding.
pub fn measure_render_mix_loudness(
    source: &Path,
    expected_sha256: &str,
    ffmpeg: &Path,
    root: &Path,
    timeout: Duration,
    flag: &AtomicBool,
    limits: crate::MixFrameLimit,
) -> Result<LoudnessAnalysis, LoudnessError> {
    check(flag)?;
    if !source.is_absolute()
        || !ffmpeg.is_absolute()
        || !root.is_absolute()
        || !fs::symlink_metadata(root)?.is_dir()
        || timeout.is_zero()
        || timeout > Duration::from_secs(3600)
        || expected_sha256.len() != 64
        || !expected_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(LoudnessError::InvalidRuntime);
    }
    let end = Instant::now() + timeout;
    let max_frames = limits.max;
    let max_bytes = max_frames * 8 + 65536;
    let mut input = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(source)?;
    let meta = input.metadata()?;
    if !meta.is_file() || meta.len() < 44 || meta.len() > max_bytes {
        return Err(LoudnessError::InvalidReport);
    }
    crate::ensure_available_space(root, meta.len() + 64 * 1024 * 1024)?;
    let directory = root.join(format!("mix-loudness-{}", uuid::Uuid::new_v4()));
    fs::DirBuilder::new().mode(0o700).create(&directory)?;
    let work = Work(directory);
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(work.0.join("source.wav"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut size = 0u64;
    loop {
        remaining(end, flag)?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > max_bytes {
            return Err(LoudnessError::InvalidReport);
        }
        digest.update(&buffer[..count]);
        out.write_all(&buffer[..count])?;
    }
    drop(out);
    if size != meta.len() || format!("{:x}", digest.finalize()) != expected_sha256 {
        return Err(LoudnessError::InvalidReport);
    }
    crate::mix_container::validate(&work.0.join("source.wav"), || {
        remaining(end, flag).map(|_| ())
    })?;
    let mut wav = hound::WavReader::open(work.0.join("source.wav"))
        .map_err(|_| LoudnessError::InvalidReport)?;
    let spec = wav.spec();
    let frames = u64::from(wav.duration());
    if spec.sample_rate != 48000
        || spec.channels != 2
        || spec.bits_per_sample != 32
        || spec.sample_format != hound::SampleFormat::Float
        || frames < limits.min
        || frames > max_frames
    {
        return Err(LoudnessError::InvalidReport);
    }
    let mut samples = 0u64;
    for value in wav.samples::<f32>() {
        if samples % 8192 == 0 {
            remaining(end, flag)?;
        }
        if !value.map_err(|_| LoudnessError::InvalidReport)?.is_finite() {
            return Err(LoudnessError::InvalidReport);
        }
        samples += 1;
    }
    if samples != frames * 2 {
        return Err(LoudnessError::InvalidReport);
    }
    drop(wav);
    let measurement = if frames < 19200 {
        LoudnessValue::NotMeasurable {
            reason: LoudnessUnavailable::TooShort,
            true_peak_db: None,
        }
    } else {
        run_captured(&work, ffmpeg, remaining(end, flag)?, flag)?
    };
    remaining(end, flag)?;
    work.finish()?;
    Ok(LoudnessAnalysis {
        version: LOUDNESS_VERSION,
        method: "ffmpeg_ebur128_summary_192k".into(),
        source_sha256: expected_sha256.into(),
        source_frames: frames,
        sample_rate: 48000,
        channels: 2,
        source_pre_gain: false,
        dual_mono: false,
        measurement,
    })
}
