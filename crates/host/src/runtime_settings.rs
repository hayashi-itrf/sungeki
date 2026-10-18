//! Per-host installation paths. Reading never mutates process environment.
use std::{
    fs::OpenOptions,
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use sungeki_protocol::{ApiError, ErrorCode};
const LIMIT: u64 = 16 * 1024;
pub(crate) use sungeki_protocol::RuntimeSettings as Settings;
fn invalid(message: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message.to_string())
}
pub fn load(database: &Path) -> Result<Settings, ApiError> {
    let path = database
        .parent()
        .ok_or_else(|| invalid("設定フォルダーがありません"))?
        .join("runtime.json");
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Settings {
                format_version: 1,
                ..Settings::default()
            });
        }
        Err(e) => return Err(ApiError::io(e)),
    };
    let metadata = file.metadata().map_err(ApiError::io)?;
    if !metadata.is_file() || metadata.len() > LIMIT {
        return Err(invalid("実行環境設定は16KiB以内の通常ファイルが必要です"));
    }
    let mut raw = Vec::new();
    (&mut file)
        .take(LIMIT + 1)
        .read_to_end(&mut raw)
        .map_err(ApiError::io)?;
    if raw.len() as u64 > LIMIT {
        return Err(invalid("実行環境設定が大きすぎます"));
    }
    // Retire two fields of this application's installation settings. This does
    // not read the predecessor application's settings or accept old MCP APIs.
    // Reading is side-effect free; the next explicit save writes current fields.
    let mut stored: serde_json::Value = serde_json::from_slice(&raw).map_err(invalid)?;
    if let Some(object) = stored.as_object_mut() {
        for key in ["python", "image_worker"] {
            if let Some(value) = object.remove(key) {
                if !value.is_null() && !value.is_string() {
                    return Err(invalid("廃止された描画設定の値が不正です"));
                }
            }
        }
    }
    let value: Settings = serde_json::from_value(stored).map_err(invalid)?;
    validate(&value)?;
    Ok(value)
}
pub fn validate(settings: &Settings) -> Result<(), ApiError> {
    if settings.format_version != 1 {
        return Err(invalid("実行環境設定の形式が未対応です"));
    }
    if let Some(root) = &settings.runtime_root {
        crate::runtime_root::mount_for(root).map_err(invalid)?;
    }
    for path in [
        &settings.voicevox_engine,
        &settings.aivis_engine,
        &settings.coeiroink_engine,
        &settings.ffmpeg,
        &settings.ffprobe,
        &settings.font,
        &settings.vrm_node,
        &settings.vrm_render_worker,
        &settings.vrm_inspect_worker,
    ]
    .into_iter()
    .flatten()
    {
        if !path.is_absolute() {
            return Err(invalid("実行環境のファイルは絶対パスで指定してください"));
        }
    }
    if let Some(hash) = &settings.font_sha256 {
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid("フォントの検査値はSHA256で指定してください"));
        }
    }
    Ok(())
}
/// An explicitly supplied override is validated, never silently replaced.
pub fn file(
    override_path: Option<PathBuf>,
    saved: Option<&Path>,
    name: &str,
) -> Result<PathBuf, ApiError> {
    let path = override_path
        .or_else(|| saved.map(Path::to_owned))
        .ok_or_else(|| {
            ApiError::new(
                ErrorCode::CapabilityUnavailable,
                format!("実行環境の設定が必要です: {name}"),
            )
        })?;
    if !path.is_absolute() || !path.is_file() {
        return Err(invalid(format!("実行環境のファイルがありません: {name}")));
    }
    Ok(path)
}
#[path = "runtime_font.rs"]
mod font;
#[path = "runtime_settings_store.rs"]
mod store;
#[cfg(test)]
#[path = "runtime_settings_tests.rs"]
mod tests;
pub(crate) fn execute(
    shared: &std::sync::Arc<std::sync::Mutex<crate::sessions::Sessions>>,
    replacement: Option<sungeki_protocol::SaveRuntimeSettingsArgs>,
) -> Result<sungeki_protocol::SessionResponse, ApiError> {
    let database = shared.lock().map_err(invalid)?.review_database_path.clone();
    let settings = match replacement {
        Some(mut args) => {
            validate(&args.settings)?;
            if let Some(root) = &args.settings.runtime_root {
                crate::runtime_root::checked(root).map_err(invalid)?;
            }
            // Hash selected font bytes outside the shared editing mutex. The
            // atomic save below still compares the original expected settings.
            font::complete(&mut args.settings)?;
            let sessions = shared.lock().map_err(invalid)?;
            ensure_storage_idle(&sessions, &args.settings)?;
            store::save(
                &sessions.review_database_path,
                &args.expected,
                args.settings,
            )?
        }
        None => load(&database)?,
    };
    Ok(sungeki_protocol::SessionResponse::RuntimeSettings { settings })
}

fn ensure_storage_idle(
    sessions: &crate::sessions::Sessions,
    replacement: &Settings,
) -> Result<(), ApiError> {
    use std::sync::atomic::Ordering;
    if load(&sessions.review_database_path)?.runtime_root == replacement.runtime_root {
        return Ok(());
    }
    if [
        &sessions.speech_runtime.active,
        &sessions.review_runtime.active,
        &sessions.build_runtime.active,
        &sessions.render_runtime.active,
        &sessions.loudness_runtime.active,
    ]
    .iter()
    .any(|active| active.load(Ordering::SeqCst) != 0)
        || sessions.speech_runtime.preparations.is_active()?
    {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "処理中は保存フォルダを変更できません。合成・出力・確認処理の終了後に保存してください。",
        ));
    }
    Ok(())
}
