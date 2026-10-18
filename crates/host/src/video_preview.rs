//! Read-only video preview. No editor lock while decoding.
#[path = "video_preview_cache.rs"]
pub(crate) mod cache;
#[path = "video_preview_guard.rs"]
mod guard;
#[path = "video_preview_proxy.rs"]
mod proxy;
use crate::{
    image_probe::video::{Admission, error},
    render_record::invalid,
    sessions::Sessions,
};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use sungeki_media::video_probe::{ProbeRuntime, frames::PreparedVideo};
use sungeki_project::{AssetLocator, AssetProbe, ClipContent};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse, VideoPreviewArgs};
struct Work {
    path: PathBuf,
    identity: (u64, u64),
}
impl Work {
    fn new(root: &Path) -> Result<Self, ApiError> {
        Self::named(root, "video-preview")
    }
    fn named(root: &Path, prefix: &str) -> Result<Self, ApiError> {
        let path = root.join(format!("{prefix}-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(invalid)?;
        let m = fs::symlink_metadata(&path).map_err(invalid)?;
        Ok(Self {
            path,
            identity: (m.dev(), m.ino()),
        })
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path)
            .is_ok_and(|m| m.is_dir() && (m.dev(), m.ino()) == self.identity)
        {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
fn remaining(end: Instant) -> Result<Duration, ApiError> {
    end.checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| ApiError::new(ErrorCode::Cancelled, "動画表示の期限を超えました"))
}
pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: VideoPreviewArgs,
) -> Result<SessionResponse, ApiError> {
    let (project, runtime, database) = {
        let sessions = shared.lock().map_err(invalid)?;
        let (_, state) = sessions.load(&args.session_id)?;
        if state.history().revision() != args.expected_revision {
            return Err(ApiError::new(ErrorCode::Conflict, "作品が更新されています"));
        }
        (
            state.history().project().clone(),
            sessions.render_runtime.clone(),
            sessions.review_database_path.clone(),
        )
    };
    let clip = project
        .clips
        .iter()
        .find(|c| c.id == args.clip_id)
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "動画クリップがありません"))?;
    let ClipContent::Video { asset_id, .. } = &clip.content else {
        return Err(invalid("動画を指定してください"));
    };
    if !project
        .tracks
        .iter()
        .any(|t| t.id == clip.track_id && t.visible)
        || args.time_us.0 < clip.start_us.0
        || args.time_us.0 >= clip.start_us.0 + clip.duration_us.0
    {
        return Err(invalid("この時刻には動画が表示されません"));
    }
    let asset = project
        .assets
        .iter()
        .find(|a| &a.id == asset_id)
        .ok_or_else(|| invalid("動画素材がありません"))?;
    let AssetProbe::Video(probe) = &asset.probe else {
        return Err(invalid("動画検査情報がありません"));
    };
    let AssetLocator::Absolute { path } = &asset.locator else {
        return Err(invalid("動画原本の絶対パスが必要です"));
    };
    let selection = proxy::Selection::load(shared, &args, asset_id, probe)?;
    let admission = Admission::acquire(runtime)?;
    let flag = &admission.runtime.stopping;
    let mut config = crate::render_runtime::preview::video(&database)?;
    config.ffprobe = config.ffprobe.canonicalize().map_err(invalid)?;
    let root = crate::render_location::root(&database)?;
    let work = Work::new(&root)?;
    let end = Instant::now() + Duration::from_secs(50);
    let key = serde_json::to_string(&(
        &args.session_id,
        asset_id,
        probe,
        path,
        &args.proxy_job_id,
        selection.as_ref().map(|s| s.identity()),
        &config.ffprobe,
    ))
    .map_err(invalid)?;
    let cached = admission
        .runtime
        .video_previews
        .lock()
        .map_err(invalid)?
        .get(&key)?;
    let prepared = if let Some(cached) = cached {
        cached
    } else {
        let source_work = Work::named(&root, "prepared-video")?;
        let mut guards = vec![(
            config.ffprobe.clone(),
            guard::OriginalStamp::read(&config.ffprobe)?,
        )];
        if let Some(selection) = &selection {
            let path = selection.manifest_path();
            guards.push((path.clone(), guard::OriginalStamp::read(&path)?));
        }
        let (video, display_path, original) = if let Some(selection) = selection.as_ref() {
            selection.open(probe, &config.ffprobe, &source_work.path, end, flag)?
        } else {
            let stamp = guard::OriginalStamp::read(Path::new(path))?;
            let video = PreparedVideo::open(
                Path::new(path),
                probe,
                &ProbeRuntime {
                    ffprobe: &config.ffprobe,
                    root: &source_work.path,
                    timeout: remaining(end)?,
                    max_source_bytes: 4 * 1024 * 1024 * 1024,
                },
                flag,
            )
            .map_err(error)?;
            (video, PathBuf::from(path), stamp)
        };
        guards.push((display_path, original));
        let path = video.snapshot_path().to_owned();
        guards.push((path.clone(), guard::OriginalStamp::read(&path)?));
        let prepared = Arc::new(cache::Prepared {
            video,
            guards,
            _work: source_work,
        });
        prepared.verify()?;
        prepared
    };
    // Clip timing (including selected original audio stream) belongs to the source.
    // The verified proxy preserves relative PTS but deliberately has no audio.
    let timing = sungeki_scene::VideoClipTiming::new(clip, probe).map_err(invalid)?;
    let frame = match timing.source_time(args.time_us) {
        Some(time) => prepared
            .video
            .decode_at(time, &config.ffmpeg, remaining(end)?, flag)
            .map_err(error)?,
        None => None,
    };
    let resource = frame
        .map(|frame| sungeki_render::stage_video_frame(frame, &work.path.join("frame.rgba"), flag))
        .transpose()
        .map_err(|e| guard::render_error(e, flag, end))?;
    let preview =
        sungeki_raster::preview::video(&serde_json::to_value(&resource).map_err(invalid)?)
            .map_err(invalid)?;
    use base64::Engine;
    let (width, height, png_base64) = (
        preview.width,
        preview.height,
        base64::engine::general_purpose::STANDARD.encode(preview.png),
    );
    guard::check(flag, end)?;
    prepared.verify()?;
    let sessions = shared.lock().map_err(invalid)?;
    let (_, state) = sessions.load(&args.session_id)?;
    if state.history().revision() != args.expected_revision {
        return Err(ApiError::new(ErrorCode::Conflict, "作品が更新されています"));
    }
    admission
        .runtime
        .video_previews
        .lock()
        .map_err(invalid)?
        .insert(key, prepared);
    Ok(SessionResponse::ImagePreview {
        asset_id: asset_id.clone(),
        sha256: probe.sha256.clone(),
        width,
        height,
        png_base64,
    })
}
