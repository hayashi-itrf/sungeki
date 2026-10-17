//! Tool subsets for source previews, without unrelated export dependencies.
use crate::runtime_settings::{self, Settings};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};
use sungeki_protocol::ApiError;

pub(crate) struct ImageRuntime {
    pub python: PathBuf,
    pub image_worker: PathBuf,
}
pub(crate) struct VideoRuntime {
    pub image: ImageRuntime,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}
fn resolve_image(
    saved: &Settings,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<ImageRuntime, ApiError> {
    Ok(ImageRuntime {
        python: runtime_settings::file(
            env("SUNGEKI_RENDER_PYTHON").map(PathBuf::from),
            saved.python.as_deref(),
            "Python",
        )?,
        image_worker: runtime_settings::image_worker(
            env("SUNGEKI_RENDER_WORKER").map(PathBuf::from),
            saved.image_worker.as_deref(),
            std::env::current_exe().ok().as_deref(),
        )?,
    })
}
fn resolve_video(
    saved: &Settings,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<VideoRuntime, ApiError> {
    Ok(VideoRuntime {
        image: resolve_image(saved, env)?,
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
pub(crate) fn image(database: &Path) -> Result<ImageRuntime, ApiError> {
    resolve_image(&runtime_settings::load(database)?, &|name| {
        std::env::var_os(name)
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
            python: Some("/missing-python".into()),
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
    #[test]
    fn source_previews_require_only_their_tools_and_keep_explicit_override_errors() {
        let fixture = crate::speech_journal_tests::Fixture::new();
        let file = fixture.0.join("tool");
        std::fs::write(&file, b"tool").unwrap();
        let mut saved = Settings {
            format_version: 1,
            python: Some(file.clone()),
            image_worker: Some(file.clone()),
            // An unrelated incomplete VRM setup must not block image previews.
            vrm_render_worker: Some("/missing-vrm-worker".into()),
            ..Default::default()
        };
        assert_eq!(resolve_image(&saved, &|_| None).unwrap().python, file);
        assert!(resolve_video(&saved, &|_| None).is_err());
        saved.ffmpeg = Some(file.clone());
        saved.ffprobe = Some(file.clone());
        assert_eq!(resolve_video(&saved, &|_| None).unwrap().ffprobe, file);
        for name in [
            "SUNGEKI_RENDER_PYTHON",
            "SUNGEKI_RENDER_WORKER",
            "SUNGEKI_RENDER_FFMPEG",
            "SUNGEKI_RENDER_FFPROBE",
        ] {
            assert!(resolve_video(&saved, &|key| (key == name).then(|| "missing".into())).is_err());
        }
        saved.python = None;
        assert!(resolve_image(&saved, &|_| None).is_err());
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
