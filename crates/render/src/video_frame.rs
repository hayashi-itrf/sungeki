//! Bridge decoded original pixels into the existing image compositor. Files are
//! private staging artifacts, never user output or replacements for originals.
use crate::{
    RenderError,
    originals::{cancelled, write_new},
    plan::error,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use sungeki_media::video_probe::frames::SourceFrame;
#[derive(Debug, Serialize)]
pub struct VideoFrameResource {
    pub path: PathBuf,
    pub sha256: String,
    pub decoded_video: VideoRaster,
}
#[derive(Debug, Serialize)]
pub struct VideoRaster {
    pub width: u32,
    pub height: u32,
    pub pts: i64,
}
/// Create a new raw RGBA artifact in a caller-owned private job directory.
/// Cancellation/error leaves cleanup to that job. Never overwrite a path.
pub fn stage_video_frame(
    frame: SourceFrame,
    target: &Path,
    cancel: &AtomicBool,
) -> Result<VideoFrameResource, RenderError> {
    cancelled(cancel)?;
    if !target.is_absolute() {
        return Err(error("動画frameの作業パスは絶対パスが必要です"));
    }
    let display = frame.into_display().map_err(error)?;
    cancelled(cancel)?;
    let parent = target
        .parent()
        .ok_or_else(|| error("動画frameの作業パスが不正です"))?;
    sungeki_media::ensure_available_space(parent, display.rgba.len() as u64)?;
    write_new(target, &display.rgba)?;
    cancelled(cancel)?;
    Ok(VideoFrameResource {
        path: target.to_owned(),
        sha256: format!("{:x}", Sha256::digest(&display.rgba)),
        decoded_video: VideoRaster {
            width: display.width,
            height: display.height,
            pts: display.pts,
        },
    })
}

/// Resolve timeline time before decoding and staging a frame. The caller must
/// select `video` from this clip's asset_id. None is outside the clip or a real
/// leading gap in the source; it must not become a stale cached image.
pub fn stage_video_clip_frame(
    video: &sungeki_media::video_probe::frames::PreparedVideo,
    clip: &sungeki_project::Clip,
    timeline_time: sungeki_project::TimeUs,
    target: &Path,
    ffmpeg: &Path,
    timeout: std::time::Duration,
    cancel: &AtomicBool,
) -> Result<Option<VideoFrameResource>, RenderError> {
    cancelled(cancel)?;
    let timing = sungeki_scene::VideoClipTiming::new(clip, video.probe()).map_err(error)?;
    let Some(time) = timing.source_time(timeline_time) else {
        return Ok(None);
    };
    let Some(frame) = video
        .decode_at(time, ffmpeg, timeout, cancel)
        .map_err(error)?
    else {
        return Ok(None);
    };
    stage_video_frame(frame, target, cancel).map(Some)
}
