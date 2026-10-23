//! Read only the connected host's saved settings and its own environment.
use super::{Settings, invalid, load, select};
use std::{
    ffi::OsString,
    path::Path,
    sync::{Arc, Mutex},
};
use sungeki_protocol::*;

pub(crate) fn execute(
    shared: &Arc<Mutex<crate::sessions::Sessions>>,
) -> Result<SessionResponse, ApiError> {
    let database = shared.lock().map_err(invalid)?.review_database_path.clone();
    let settings = load(&database)?;
    let environment = describe(&database, &settings, |name| std::env::var_os(name))?;
    Ok(SessionResponse::RuntimeEnvironment { environment })
}

fn describe(
    database: &Path,
    saved: &Settings,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<RuntimeEnvironment, ApiError> {
    // Do not call data::directory(): it reevaluates caller-independent defaults
    // and creates storage. The database belongs to the already connected host.
    let data_directory = database
        .parent()
        .ok_or_else(|| invalid("設定フォルダーがありません"))?
        .to_owned();
    let values = RuntimeEnvironmentValues {
        runtime_root: saved
            .runtime_root
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned()),
        ffmpeg: saved
            .ffmpeg
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned()),
        ffprobe: saved
            .ffprobe
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned()),
        font: saved
            .font
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned()),
        font_sha256: saved.font_sha256.clone(),
    };
    let selected = RuntimeEnvironmentSelections {
        runtime_root: selection("SUNGEKI_RUNTIME_ROOT", values.runtime_root.as_deref(), &env)?,
        ffmpeg: selection("SUNGEKI_RENDER_FFMPEG", values.ffmpeg.as_deref(), &env)?,
        ffprobe: selection("SUNGEKI_RENDER_FFPROBE", values.ffprobe.as_deref(), &env)?,
        font: selection("SUNGEKI_RENDER_FONT", values.font.as_deref(), &env)?,
        font_sha256: selection(
            "SUNGEKI_RENDER_FONT_SHA256",
            values.font_sha256.as_deref(),
            &env,
        )?,
    };
    Ok(RuntimeEnvironment {
        runtime_settings_path: data_directory.join("runtime.json"),
        data_directory,
        scope: RuntimeEnvironmentScope::RenderPathsAndRuntimeRoot,
        saved: values,
        selected,
        verification: RuntimeEnvironmentVerification::NotRun,
    })
}

fn selection(
    name: &str,
    saved: Option<&str>,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<RuntimeSettingSelection, ApiError> {
    let (value, source) = select(env(name), saved.map(OsString::from));
    let value = value
        .map(|value| {
            value.into_string().map_err(|_| {
                let mut error = invalid("実行環境の値はUTF-8で指定してください");
                error
                    .details
                    .insert("reason".into(), "runtime_value_encoding".into());
                error
                    .details
                    .insert("environment_variable".into(), name.into());
                error
            })
        })
        .transpose()?;
    Ok(RuntimeSettingSelection {
        value,
        source,
        environment_variable: name.into(),
    })
}

#[cfg(test)]
#[path = "runtime_environment_tests.rs"]
mod tests;
