#[path = "psd_preview_cache.rs"]
mod cache;
use crate::{render_record::invalid, sessions::Sessions};
use std::{
    path::Path,
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::Duration,
};
use sungeki_project::{AssetLocator, AssetProbe, ClipContent, PsdProbe, TimeUs};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse};

static PREVIEWING: Mutex<()> = Mutex::new(());

pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    expected_revision: u32,
    clip_id: String,
    time_us: TimeUs,
) -> Result<SessionResponse, ApiError> {
    let _permit = PREVIEWING.try_lock().map_err(|_| {
        let mut error = ApiError::new(ErrorCode::Conflict, "別のPSDを表示用に描画中です");
        error.retryable = true;
        error
    })?;
    let (project, database) = {
        let sessions = shared.lock().map_err(invalid)?;
        let (_, state) = sessions.load(&session_id)?;
        if state.history().revision() != expected_revision {
            return Err(ApiError::new(ErrorCode::Conflict, "作品が更新されています"));
        }
        (
            state.history().project().clone(),
            sessions.review_database_path.clone(),
        )
    };
    let clip = project
        .clips
        .iter()
        .find(|clip| clip.id == clip_id)
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "クリップがありません"))?;
    if !matches!(clip.content, ClipContent::Psd { .. }) {
        return Err(invalid("PSD立ち絵を指定してください"));
    }
    let plan = sungeki_scene::ScenePlan::new(&project).map_err(invalid)?;
    let layer = plan
        .layers_at(time_us)
        .find(|layer| layer.clip_id == clip_id)
        .ok_or_else(|| invalid("この時刻には立ち絵が表示されません"))?;
    let sungeki_scene::Primitive::Psd { asset_id, pose } = &layer.primitive else {
        return Err(invalid("PSDの描画計画がありません"));
    };
    let asset = project
        .assets
        .iter()
        .find(|asset| &asset.id == asset_id)
        .ok_or_else(|| invalid("PSD素材がありません"))?;
    let AssetProbe::Psd(probe) = &asset.probe else {
        return Err(invalid("PSD検査情報がありません"));
    };
    let AssetLocator::Absolute { path } = &asset.locator else {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "未対応の素材参照です",
        ));
    };
    let config = crate::render_runtime::preview::image(&database)?;
    let worker = config
        .image_worker
        .parent()
        .ok_or_else(|| invalid("画像workerの配置が不正です"))?
        .join("psd_preview.py");
    let key = cache::key(Path::new(path), probe, pose, &worker, &config.python)?;
    if let Some(cached) = cache::get(&key)? {
        return publish(
            shared,
            &session_id,
            expected_revision,
            SessionResponse::ImagePreview {
                asset_id: asset_id.clone(),
                sha256: probe.sha256.clone(),
                width: cached.width,
                height: cached.height,
                png_base64: cached.png,
            },
        );
    }
    let args = vec![
        worker.into_os_string(),
        "--source".into(),
        path.into(),
        "--sha256".into(),
        probe.sha256.clone().into(),
        "--pose".into(),
        serde_json::to_string(pose).map_err(invalid)?.into(),
    ];
    let result = sungeki_media::run_process(
        &sungeki_media::ProcessSpec {
            program: &config.python,
            args: &args,
            directory: Path::new(path)
                .parent()
                .ok_or_else(|| invalid("PSD親フォルダーがありません"))?,
            timeout: Duration::from_secs(60),
            max_output_bytes: 12 * 1024 * 1024,
        },
        &AtomicBool::new(false),
    )
    .map_err(invalid)?;
    if !result.succeeded() {
        return Err(invalid(format!(
            "PSD表示の描画に失敗: {:?}: {}",
            result.stopped,
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    #[derive(serde::Deserialize)]
    struct Preview {
        probe: PsdProbe,
        width: u32,
        height: u32,
        png_base64: String,
    }
    let rendered: Preview = serde_json::from_slice(&result.stdout).map_err(invalid)?;
    if &rendered.probe != probe
        || !(1..=1024).contains(&rendered.width)
        || !(1..=1024).contains(&rendered.height)
    {
        return Err(invalid("PSD原本の検査結果または表示寸法が一致しません"));
    }
    // Revalidate again after worker completion before publishing/reusing pixels.
    cache::verify(Path::new(path), probe)?;
    cache::insert(
        key,
        cache::Pixels {
            width: rendered.width,
            height: rendered.height,
            png: rendered.png_base64.clone(),
        },
    )?;
    publish(
        shared,
        &session_id,
        expected_revision,
        SessionResponse::ImagePreview {
            asset_id: asset_id.clone(),
            sha256: probe.sha256.clone(),
            width: rendered.width,
            height: rendered.height,
            png_base64: rendered.png_base64,
        },
    )
}

// Decoding and cache validation run outside the editor lock. An edit may have
// arrived during either operation; never publish a successful stale preview.
fn publish(
    shared: &Arc<Mutex<Sessions>>,
    session_id: &str,
    expected_revision: u32,
    response: SessionResponse,
) -> Result<SessionResponse, ApiError> {
    let sessions = shared.lock().map_err(invalid)?;
    let (_, state) = sessions.load(session_id)?;
    if state.history().revision() != expected_revision {
        return Err(ApiError::new(ErrorCode::Conflict, "作品が更新されています"));
    }
    Ok(response)
}
