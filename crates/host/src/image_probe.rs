#[path = "video_probe.rs"]
pub(crate) mod video;
#[path = "video_preview.rs"]
pub(crate) mod video_preview;
use std::{path::Path, sync::Mutex};
use sungeki_assets::{ProbeError, ProbeLimits, probe_image};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse};

// Decoding is outside the session lock, but has a separate process-wide admission limit.
static PROBING: Mutex<()> = Mutex::new(());
pub fn execute(path: String) -> Result<SessionResponse, ApiError> {
    let _permit = permit()?;
    probe_image(Path::new(&path), ProbeLimits::default())
        .map(|probe| SessionResponse::ImageProbed { probe })
        .map_err(|cause| error(cause, path))
}
fn error(cause: ProbeError, path: String) -> ApiError {
    let code = match &cause {
        ProbeError::Io(e) if e.kind() == std::io::ErrorKind::NotFound => ErrorCode::AssetMissing,
        ProbeError::Io(e) if e.kind() == std::io::ErrorKind::InvalidInput => {
            ErrorCode::ValidationFailed
        }
        ProbeError::Io(_) => ErrorCode::IoError,
        ProbeError::UnsupportedFormat | ProbeError::AnimatedImage => {
            ErrorCode::CapabilityUnavailable
        }
        ProbeError::SourceMismatch => ErrorCode::ExternalModification,
        ProbeError::SourceChanged => ErrorCode::Conflict,
        _ => ErrorCode::ValidationFailed,
    };
    let mut error = ApiError::new(code, cause.to_string());
    error.retryable = matches!(cause, ProbeError::SourceChanged);
    error.details.insert("path".into(), path);
    error
}

fn permit() -> Result<std::sync::MutexGuard<'static, ()>, ApiError> {
    PROBING.try_lock().map_err(|_| {
        let mut error = ApiError::new(
            ErrorCode::Conflict,
            "別の画像を検査中です。再試行してください",
        );
        error.retryable = true;
        error
    })
}
pub fn preview(asset: sungeki_project::Asset) -> Result<SessionResponse, ApiError> {
    use base64::Engine;
    let _permit = permit()?;
    let sungeki_project::AssetProbe::Image(probe) = asset.probe else {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "画像素材を指定してください",
        ));
    };
    let sungeki_project::AssetLocator::Absolute { path } = asset.locator else {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "未対応の素材参照です",
        ));
    };
    let (png, width, height) = sungeki_assets::image_preview(Path::new(&path), &probe.sha256)
        .map_err(|cause| error(cause, path))?;
    Ok(SessionResponse::ImagePreview {
        asset_id: asset.id,
        sha256: probe.sha256,
        png_base64: base64::engine::general_purpose::STANDARD.encode(png),
        width,
        height,
    })
}
