//! Tool subsets for source previews, without unrelated export dependencies.
use crate::runtime_settings::{self, Settings};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};
use sungeki_protocol::ApiError;

pub(crate) struct VideoRuntime {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}
fn resolve_video(
    saved: &Settings,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<VideoRuntime, ApiError> {
    Ok(VideoRuntime {
        ffmpeg: runtime_settings::file(
            env("SUNGEKI_RENDER_FFMPEG").map(PathBuf::from),
            saved.ffmpeg.as_deref(),
            "FFmpeg",
        )?,
        ffprobe: runtime_settings::file(
            env("SUNGEKI_RENDER_FFPROBE").map(PathBuf::from),
            saved.ffprobe.as_deref(),
            "FFprobe",
        )?,
    })
}
pub(crate) fn video(database: &Path) -> Result<VideoRuntime, ApiError> {
    resolve_video(&runtime_settings::load(database)?, &|name| {
        std::env::var_os(name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_tools_do_not_require_visual_setup_and_reject_bad_audio_overrides() {
        let fixture = crate::speech_journal_tests::Fixture::new();
        let file = fixture.0.join("tool");
        std::fs::write(&file, b"tool").unwrap();
        let saved = Settings {
            format_version: 1,
            ffmpeg: Some(file.clone()),
            ffprobe: Some(file.clone()),

            font: Some("/missing-font".into()),
            vrm_render_worker: Some("/missing-vrm".into()),
            ..Default::default()
        };
        assert_eq!(resolve_audio(&saved, &|_| None).unwrap().ffmpeg, file);
        for key in ["SUNGEKI_RENDER_FFMPEG", "SUNGEKI_RENDER_FFPROBE"] {
            assert!(
                resolve_audio(&saved, &|name| (name == key)
                    .then(|| "/missing-tool".into()))
                .is_err()
            );
        }
        assert!(resolve_audio(&Settings::default(), &|_| None).is_err());
    }
}

pub(crate) fn audio(database: &Path) -> Result<sungeki_render::AudioRuntime, ApiError> {
    resolve_audio(&runtime_settings::load(database)?, &|name| {
        std::env::var_os(name)
    })
}
fn resolve_audio(
    saved: &Settings,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<sungeki_render::AudioRuntime, ApiError> {
    Ok(sungeki_render::AudioRuntime {
        ffmpeg: runtime_settings::file(
            env("SUNGEKI_RENDER_FFMPEG").map(PathBuf::from),
            saved.ffmpeg.as_deref(),
            "FFmpeg",
        )?,
        ffprobe: runtime_settings::file(
            env("SUNGEKI_RENDER_FFPROBE").map(PathBuf::from),
            saved.ffprobe.as_deref(),
            "FFprobe",
        )?,
        deadline: std::time::Duration::from_secs(7200),
    })
}
