pub(crate) use crate::media_job_runtime::Runtime;
#[path = "preview_runtime.rs"]
pub(crate) mod preview;
use crate::render_record::invalid;
use std::{path::PathBuf, time::Duration};
use sungeki_protocol::{ApiError, ErrorCode};
pub(crate) fn configuration(
    database: &std::path::Path,
    project: &sungeki_project::Project,
) -> Result<sungeki_render::RenderRuntime, ApiError> {
    let saved = crate::runtime_settings::load(database)?;
    resolve(&saved, needs_vrm(project), |name| std::env::var_os(name))
}
fn resolve(
    saved: &crate::runtime_settings::Settings,
    needs_vrm: bool,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<sungeki_render::RenderRuntime, ApiError> {
    let path = |name: &str, value: Option<&std::path::Path>| {
        crate::runtime_settings::file(env(name).map(PathBuf::from), value, name)
    };
    let font_sha256 = match env("SUNGEKI_RENDER_FONT_SHA256") {
        Some(value) => value
            .into_string()
            .map_err(|_| invalid("フォント検査値が不正です"))?,
        None => saved.font_sha256.clone().ok_or_else(|| {
            ApiError::new(
                ErrorCode::CapabilityUnavailable,
                "字幕フォントの準備が必要です。「設定」→「動画制作の実行環境」で保存済み設定を読み込み、字幕フォントを選んで保存してください。検査値は保存時に自動取得します。初回は画像処理用Python・FFmpeg・FFprobeも設定してください。",
            )
        })?,
    };
    crate::runtime_settings::validate(&crate::runtime_settings::Settings {
        format_version: 1,
        font_sha256: Some(font_sha256.clone()),
        ..Default::default()
    })?;
    Ok(sungeki_render::RenderRuntime {
        vrm: if needs_vrm {
            Some(sungeki_render::vrm_worker::VrmRuntime {
                node: path("SUNGEKI_VRM_NODE", saved.vrm_node.as_deref())?,
                worker: path(
                    "SUNGEKI_VRM_RENDER_WORKER",
                    saved.vrm_render_worker.as_deref(),
                )?,
                work_root: crate::runtime_root::selected(
                    env("SUNGEKI_RUNTIME_ROOT"),
                    saved.runtime_root.as_deref(),
                )
                .map_err(|error| {
                    ApiError::new(ErrorCode::CapabilityUnavailable, error.to_string())
                })?
                .join("vrm-work"),
            })
        } else {
            None
        },
        python: path("SUNGEKI_RENDER_PYTHON", saved.python.as_deref())?,
        image_worker: crate::runtime_settings::image_worker(
            env("SUNGEKI_RENDER_WORKER").map(PathBuf::from),
            saved.image_worker.as_deref(),
            std::env::current_exe().ok().as_deref(),
        )?,
        ffmpeg: path("SUNGEKI_RENDER_FFMPEG", saved.ffmpeg.as_deref())?,
        ffprobe: path("SUNGEKI_RENDER_FFPROBE", saved.ffprobe.as_deref())?,
        font: path("SUNGEKI_RENDER_FONT", saved.font.as_deref())?,
        font_sha256,
        deadline: Duration::from_secs(7200),
    })
}

#[cfg(test)]
#[path = "render_runtime_tests.rs"]
mod tests;

fn needs_vrm(project: &sungeki_project::Project) -> bool {
    project.clips.iter().any(|clip| {
        matches!(clip.content, sungeki_project::ClipContent::Vrm { .. })
            && project
                .tracks
                .iter()
                .any(|track| track.id == clip.track_id && track.visible)
    })
}
