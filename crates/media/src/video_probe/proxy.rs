//! Lossless reduced-raster editing proxy. Audio stays on the original audio path.
use super::{PreparedVideo, run};
use crate::{
    proxy_timeline::ProxyTimeline,
    video_probe::{ProbeError, ProbeRuntime, check, probe},
};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};
use sungeki_project::{MediaRatio, VideoDisplayGeometry, VideoStreamInfo};

/// Bounded SSD artifact budget; snapshots remain streamed, never buffered whole.
pub const MAX_PROXY_BYTES: u64 = 1024 * 1024 * 1024;
#[path = "proxy_settings.rs"]
mod settings;
pub use settings::{ProxyEncoding, ProxySettings};
/// Only returned after independent probe and full presentation timeline comparison.
/// This is a transient owned snapshot, not yet a persisted or indexed cache entry.
pub struct GeneratedProxy {
    pub video: PreparedVideo,
    pub timeline: ProxyTimeline,
}
impl GeneratedProxy {
    /// Read-only input for verified cache publication. Valid until this owner drops;
    /// never modify or retain the path as the published cache location.
    pub fn snapshot_path(&self) -> &Path {
        &self.video.snapshot.path
    }
}
struct Output(PathBuf);
impl Drop for Output {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl PreparedVideo {
    /// Revalidate a cached proxy against a verified original presentation map.
    pub fn verify_proxy(
        &self,
        source: &super::ProbedVideo,
        pts: &[i64],
        settings: ProxySettings,
    ) -> Result<ProxyTimeline, ProbeError> {
        source
            .validate()
            .map_err(|_| ProbeError::Invalid("invalid proxy original"))?;
        let (width, height) = dimensions(&source.metadata.video, settings)?;
        verify_display(
            &source.metadata.video,
            &self.probe.metadata.video,
            width,
            height,
            settings.encoding,
        )?;
        if !self.probe.metadata.audio.is_empty() {
            return Err(ProbeError::Invalid("unexpected proxy audio"));
        }
        ProxyTimeline::verify(
            &source.metadata.video.clock,
            pts,
            &self.probe.metadata.video.clock,
            self.frame_pts(),
        )
        .map_err(ProbeError::Invalid)
    }
    pub fn generate_proxy(
        &self,
        settings: ProxySettings,
        ffmpeg: &Path,
        runtime: &ProbeRuntime<'_>,
        cancel: &AtomicBool,
    ) -> Result<GeneratedProxy, ProbeError> {
        settings.descriptor()?;
        if !ffmpeg.is_absolute() {
            return Err(ProbeError::Invalid("proxy encoder path"));
        }
        super::validate_runtime(&self.snapshot.path, runtime)?;
        let deadline = Instant::now() + runtime.timeout;
        check(cancel, deadline)?;
        let source = &self.probe.metadata.video;
        let (width, height) = dimensions(source, settings)?;
        if source.clock.time_base.denominator > i32::MAX as u32 {
            return Err(ProbeError::Invalid("proxy container time base"));
        }
        crate::ensure_available_space(&self.snapshot.root, 4 * MAX_PROXY_BYTES + 1024 * 1024)?;
        let output = Output(
            self.snapshot
                .root
                .join(format!("proxy-{}.mov", uuid::Uuid::new_v4())),
        );
        let mut args: Vec<OsString> = [
            "-nostdin",
            "-v",
            "error",
            "-xerror",
            "-n",
            "-protocol_whitelist",
            "file",
            "-format_whitelist",
            "mov,matroska,webm",
            "-copyts",
            "-noautorotate",
            "-i",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        args.push(self.snapshot.path.as_os_str().to_owned());
        args.extend([
            "-map".into(),
            format!("0:{}", source.index).into(),
            "-vf".into(),
            format!("scale={width}:{height}:flags=bilinear").into(),
        ]);
        args.extend(
            [
                "-an",
                "-sn",
                "-dn",
                "-c:v",
                settings.encoding.codec(),
                "-pix_fmt",
                settings.encoding.pixels(),
                "-threads",
                "1",
                "-fps_mode",
                "passthrough",
                "-enc_time_base",
                "-1",
                "-avoid_negative_ts",
                "disabled",
                "-video_track_timescale",
            ]
            .into_iter()
            .map(Into::into),
        );
        args.push(source.clock.time_base.denominator.to_string().into());
        if settings.encoding == ProxyEncoding::Qtrle {
            args.extend(["-g".into(), "30".into()]);
        }
        args.extend([
            "-fs".into(),
            MAX_PROXY_BYTES.to_string().into(),
            "-f".into(),
            "mov".into(),
            output.0.as_os_str().to_owned(),
        ]);
        run(&self.snapshot, ffmpeg, &args, deadline, 1024 * 1024, cancel)?;
        // Encoding discards the MOV display matrix. Reattach rotation without
        // re-encoding; the full matrix is compared below (reflections that cannot
        // be represented by this path are rejected, never silently flattened).
        let rotated = if let Some(angle) = source.rotation_degrees {
            let rotated = Output(
                self.snapshot
                    .root
                    .join(format!("proxy-{}.mov", uuid::Uuid::new_v4())),
            );
            let args: Vec<OsString> = [
                "-nostdin",
                "-v",
                "error",
                "-xerror",
                "-n",
                "-protocol_whitelist",
                "file",
                "-format_whitelist",
                "mov",
                "-copyts",
                "-display_rotation",
            ]
            .into_iter()
            .map(Into::into)
            .chain([
                angle.to_string().into(),
                "-i".into(),
                output.0.as_os_str().to_owned(),
                "-map".into(),
                "0:v:0".into(),
                "-c".into(),
                "copy".into(),
                "-avoid_negative_ts".into(),
                "disabled".into(),
                "-video_track_timescale".into(),
                source.clock.time_base.denominator.to_string().into(),
                "-fs".into(),
                MAX_PROXY_BYTES.to_string().into(),
                "-f".into(),
                "mov".into(),
                rotated.0.as_os_str().to_owned(),
            ])
            .collect();
            run(&self.snapshot, ffmpeg, &args, deadline, 1024 * 1024, cancel)?;
            Some(rotated)
        } else {
            None
        };
        let result_path = &rotated.as_ref().unwrap_or(&output).0;
        if std::fs::metadata(result_path)?.len() > MAX_PROXY_BYTES {
            return Err(ProbeError::Invalid("proxy exceeds 1GiB cache limit"));
        }
        let remaining = || -> Result<ProbeRuntime<'_>, ProbeError> {
            check(cancel, deadline)?;
            Ok(ProbeRuntime {
                ffprobe: runtime.ffprobe,
                root: runtime.root,
                timeout: deadline.saturating_duration_since(Instant::now()),
                max_source_bytes: MAX_PROXY_BYTES,
            })
        };
        let verified = probe(result_path, &remaining()?, cancel)?;
        let video = PreparedVideo::open(result_path, &verified, &remaining()?, cancel)?;
        let timeline = video.verify_proxy(&self.probe, self.frame_pts(), settings)?;
        check(cancel, deadline)?;
        Ok(GeneratedProxy { video, timeline })
    }
}

fn verify_display(
    source: &VideoStreamInfo,
    proxy: &VideoStreamInfo,
    width: u32,
    height: u32,
    encoding: ProxyEncoding,
) -> Result<(), ProbeError> {
    let sar = |s: &VideoStreamInfo| {
        s.sample_aspect_ratio.unwrap_or(MediaRatio {
            numerator: 1,
            denominator: 1,
        })
    };
    let a = sar(source);
    let b = sar(proxy);
    let lhs = u128::from(source.width)
        * u128::from(a.numerator)
        * u128::from(proxy.height)
        * u128::from(b.denominator);
    let rhs = u128::from(proxy.width)
        * u128::from(b.numerator)
        * u128::from(source.height)
        * u128::from(a.denominator);
    if proxy.width != width
        || proxy.height != height
        || proxy.codec != encoding.codec()
        || lhs != rhs
        || source.rotation_degrees != proxy.rotation_degrees
        || source.display_matrix != proxy.display_matrix
        || source.legacy_rotate_tag != proxy.legacy_rotate_tag
    {
        return Err(ProbeError::Invalid("proxy display geometry changed"));
    }
    VideoDisplayGeometry::new(proxy).map_err(ProbeError::Invalid)?;
    Ok(())
}

fn dimensions(source: &VideoStreamInfo, settings: ProxySettings) -> Result<(u32, u32), ProbeError> {
    settings.descriptor()?;
    let geometry = VideoDisplayGeometry::new(source).map_err(ProbeError::Invalid)?;
    let factor =
        (f64::from(settings.max_edge) / f64::from(geometry.width.max(geometry.height))).min(1.0);
    Ok((
        (f64::from(source.width) * factor).floor().max(1.0) as u32,
        (f64::from(source.height) * factor).floor().max(1.0) as u32,
    ))
}
