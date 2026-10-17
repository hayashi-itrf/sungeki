use crate::render_record::invalid;
use std::{
    path::Path,
    sync::{Mutex, atomic::AtomicBool},
    time::Duration,
};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse};

static PROBING: Mutex<()> = Mutex::new(());
pub(crate) fn execute(database: &Path, path: String) -> Result<SessionResponse, ApiError> {
    let _permit = PROBING
        .try_lock()
        .map_err(|_| ApiError::new(ErrorCode::Conflict, "別のPSDを検査中です"))?;
    let source = Path::new(&path);
    let (sha256, byte_length) =
        sungeki_assets::image_source_hash(source, Default::default()).map_err(invalid)?;
    let config = crate::render_runtime::preview::image(database)?;
    let worker = config
        .image_worker
        .parent()
        .ok_or_else(|| invalid("画像workerの配置が不正です"))?
        .join("psd_pose.py");
    let args = vec![
        worker.into_os_string(),
        "--source".into(),
        source.into(),
        "--sha256".into(),
        sha256.clone().into(),
    ];
    let result = sungeki_media::run_process(
        &sungeki_media::ProcessSpec {
            program: &config.python,
            args: &args,
            directory: source
                .parent()
                .ok_or_else(|| invalid("PSD親フォルダーがありません"))?,
            timeout: Duration::from_secs(60),
            max_output_bytes: 4 * 1024 * 1024,
        },
        &AtomicBool::new(false),
    )
    .map_err(invalid)?;
    if !result.succeeded() {
        return Err(invalid(format!(
            "PSD検査失敗: {:?}: {}",
            result.stopped,
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).map_err(invalid)?;
    let probe: sungeki_project::PsdProbe =
        serde_json::from_value(report["probe"].clone()).map_err(invalid)?;
    probe.validate().map_err(invalid)?;
    if probe.sha256 != sha256 || probe.byte_length != byte_length {
        return Err(invalid("PSD原本の検査結果が一致しません"));
    }
    Ok(SessionResponse::PsdProbed { probe })
}
