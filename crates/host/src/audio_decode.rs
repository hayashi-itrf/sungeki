use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use sungeki_media::{AudioDecodeError, AudioDecodeRuntime, Mp3DecodeError, StopReason};
use sungeki_protocol::{ApiError, ErrorCode};
pub(crate) struct Configuration {
    ffmpeg: PathBuf,
    root: PathBuf,
}
impl Configuration {
    pub fn load(database: &Path) -> Result<Self, ApiError> {
        let saved = crate::runtime_settings::load(database)?;
        let ffmpeg = crate::runtime_settings::file(
            std::env::var_os("SUNGEKI_RENDER_FFMPEG").map(PathBuf::from),
            saved.ffmpeg.as_deref(),
            "FFmpeg",
        )?;
        Ok(Self {
            ffmpeg,
            root: crate::render_location::root(database)?,
        })
    }
    pub fn runtime(&self) -> AudioDecodeRuntime<'_> {
        AudioDecodeRuntime {
            ffmpeg: &self.ffmpeg,
            root: &self.root,
            timeout: Duration::from_secs(60),
        }
    }
}
pub(crate) fn error(cause: AudioDecodeError) -> ApiError {
    let code = match &cause {
        AudioDecodeError::Source(_) => {
            if let AudioDecodeError::Source(source) = cause {
                return crate::audio_analysis::error(source);
            }
            unreachable!()
        }
        AudioDecodeError::Mismatch => ErrorCode::ExternalModification,
        AudioDecodeError::MissingRuntime => ErrorCode::CapabilityUnavailable,
        AudioDecodeError::Mp3(
            Mp3DecodeError::Cancelled | Mp3DecodeError::Stopped(StopReason::Cancelled),
        ) => ErrorCode::Cancelled,
        AudioDecodeError::Mp3(Mp3DecodeError::Io(_)) => ErrorCode::IoError,
        _ => ErrorCode::ValidationFailed,
    };
    ApiError::new(code, cause.to_string())
}
