//! Canonical audio aligned to the captured video's origin, including silence.
use super::{PreparedVideo, ProbeError, check};
use crate::{ProcessSpec, WavInfo};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
pub struct VideoAudioFile<'a> {
    path: PathBuf,
    info: WavInfo,
    _source: &'a PreparedVideo,
}
impl VideoAudioFile<'_> {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn info(&self) -> &WavInfo {
        &self.info
    }
}
impl Drop for VideoAudioFile<'_> {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
impl PreparedVideo {
    /// Output sample zero is the video origin, not the first audio packet.
    /// Exact video duration is rounded up to 48kHz; maximum two hours.
    pub fn decode_audio(
        &self,
        index: u32,
        ffmpeg: &Path,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<VideoAudioFile<'_>, ProbeError> {
        if !ffmpeg.is_absolute() || timeout.is_zero() || timeout > Duration::from_secs(300) {
            return Err(ProbeError::Invalid("audio runtime"));
        }
        let end = Instant::now() + timeout;
        check(cancel, end)?;
        let audio = self
            .probe
            .metadata
            .audio
            .iter()
            .find(|s| s.index == index)
            .ok_or(ProbeError::Invalid("unknown audio stream"))?;
        if audio.clock.start_pts.is_none() {
            return Err(ProbeError::Invalid("unknown audio origin"));
        }
        let clock = &self.probe.metadata.video.clock;
        let start = clock
            .start_pts
            .ok_or(ProbeError::Invalid("unknown video origin"))?;
        let ticks = clock
            .duration_ts
            .ok_or(ProbeError::Invalid("unknown video duration"))?;
        let numerator = i128::from(start) * i128::from(clock.time_base.numerator) * 48000;
        let denominator = i128::from(clock.time_base.denominator);
        let origin = (numerator.abs() + denominator / 2) / denominator * numerator.signum();
        if origin.abs() > 1_i128 << 52 {
            return Err(ProbeError::Invalid("audio origin precision limit"));
        }
        let frames = (u128::from(ticks) * u128::from(clock.time_base.numerator) * 48000)
            .div_ceil(u128::from(clock.time_base.denominator));
        if frames == 0 || frames > 48000 * 7200 {
            return Err(ProbeError::Invalid("audio duration limit"));
        }
        let frames = frames as u64;
        let size = frames * 8;
        crate::ensure_available_space(&self.snapshot.root, size + 44)?;
        let path = self
            .snapshot
            .root
            .join(format!("audio-{}.wav", uuid::Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let mut result = VideoAudioFile {
            path,
            info: WavInfo {
                sample_rate: 48000,
                channels: 2,
                bits_per_sample: 32,
                is_float: true,
                frames,
                duration_us: (frames * 1_000_000).div_ceil(48000),
                content_hash: String::new(),
            },
            _source: self,
        };
        file.write_all(&crate::mp3_decode::pcm_header(size as u32))?;
        // Preserve sub-100ms timestamp gaps; the resampler default would
        // otherwise leave short gaps uncompensated and shift later audio.
        let filter = format!(
            "aresample=48000,asettb=1/48000,asetpts=PTS-({origin}),aresample=48000:async=1:min_comp=0:min_hard_comp=0:first_pts=0,apad,atrim=end_sample={frames}"
        );
        let args: Vec<OsString> = [
            "-nostdin",
            "-v",
            "error",
            "-xerror",
            "-copyts",
            "-protocol_whitelist",
            "file",
            "-format_whitelist",
            "mov,matroska,webm",
            "-i",
        ]
        .into_iter()
        .map(Into::into)
        .chain([self.snapshot.path.as_os_str().to_owned()])
        .chain([
            "-map".into(),
            format!("0:{index}").into(),
            "-af".into(),
            filter.into(),
        ])
        .chain(
            [
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
            .map(Into::into),
        )
        .collect();
        let output = crate::run_process_to_writer(
            &ProcessSpec {
                program: ffmpeg,
                args: &args,
                directory: &self.snapshot.root,
                timeout: end.saturating_duration_since(Instant::now()),
                max_output_bytes: 65536,
            },
            cancel,
            &mut file,
            size as usize,
        )?;
        check(cancel, end)?;
        if let Some(stop) = output.stopped {
            return Err(ProbeError::Stopped(stop));
        }
        if !output.status.success() || !output.stderr.is_empty() {
            return Err(ProbeError::Worker(
                String::from_utf8_lossy(&output.stderr)
                    .chars()
                    .take(2048)
                    .collect(),
            ));
        }
        if file.metadata()?.len() != size + 44 {
            return Err(ProbeError::Invalid("incomplete video audio"));
        }
        file.seek(SeekFrom::Start(44))?;
        let mut buffer = [0u8; 65536];
        let mut remaining = size;
        while remaining > 0 {
            check(cancel, end)?;
            let n = remaining.min(buffer.len() as u64) as usize;
            file.read_exact(&mut buffer[..n])?;
            if buffer[..n]
                .chunks_exact(4)
                .any(|b| !f32::from_le_bytes(b.try_into().unwrap()).is_finite())
            {
                return Err(ProbeError::Invalid("non-finite audio sample"));
            }
            remaining -= n as u64;
        }
        file.seek(SeekFrom::Start(0))?;
        let mut digest = Sha256::new();
        loop {
            check(cancel, end)?;
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            digest.update(&buffer[..n]);
        }
        file.sync_all()?;
        result.info.content_hash = format!("{:x}", digest.finalize());
        Ok(result)
    }
}
