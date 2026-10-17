//! Preparation inspection, not a Prepare completion receipt or quality review.
use crate::{build_state::invalid, sessions::Sessions};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use sungeki_project::{AssetLocator, AssetProbe, BuildRecipe};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse};

/// Blocking and bounded. Call outside the session lock. Reuses normal import
/// inspection, including decoding/probing rather than trusting recipe metadata.
pub(crate) fn verify(
    shared: &Arc<Mutex<Sessions>>,
    recipe: &BuildRecipe,
    mut cancelled: impl FnMut() -> bool,
) -> Result<usize, ApiError> {
    recipe.validate().map_err(invalid)?;
    let mut total = 0u64;
    for asset in &recipe.assets {
        let length = match &asset.probe {
            AssetProbe::Video(p) => p.byte_length,
            AssetProbe::Vrm(p) => p.byte_length,
            AssetProbe::Image(p) => p.byte_length,
            AssetProbe::Psd(p) => p.byte_length,
            AssetProbe::Audio(p) => p.byte_length,
        };
        total = total
            .checked_add(length)
            .ok_or_else(|| invalid("素材容量上限です"))?;
        let limit = if matches!(asset.probe, AssetProbe::Video(_)) {
            4 * 1024 * 1024 * 1024
        } else if matches!(asset.probe, AssetProbe::Vrm(_)) {
            100 * 1024 * 1024
        } else {
            64 * 1024 * 1024
        };
        if length == 0 || length > limit || total > 1024 * 1024 * 1024 {
            return Err(invalid(
                "制作素材は合計1GiB以内、VRMは1原本100MiB・その他の動画以外は64MiB以内です",
            ));
        }
    }
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut stop = || cancelled() || Instant::now() >= deadline;
    for asset in &recipe.assets {
        if stop() {
            return Err(ApiError::new(
                ErrorCode::Cancelled,
                "素材検査を中断しました",
            ));
        }
        let AssetLocator::Absolute { path } = &asset.locator else {
            return Err(invalid("制作素材は解決済みの絶対パスが必要です"));
        };
        if !Path::new(path).is_absolute() {
            return Err(invalid("素材パスが不正です"));
        }
        let (length, actual) = match &asset.probe {
            AssetProbe::Vrm(p) => {
                let SessionResponse::VrmProbed { probe } =
                    crate::image_probe::video::vrm_import::execute(
                        shared,
                        path.clone(),
                        p.inspection.kind,
                    )?
                else {
                    return Err(invalid("VRM検査結果が不正です"));
                };
                (p.byte_length, AssetProbe::Vrm(probe))
            }
            AssetProbe::Video(p) => {
                let SessionResponse::VideoProbed { probe } =
                    crate::image_probe::video::execute_cancellable(
                        shared,
                        path.clone(),
                        &mut stop,
                    )?
                else {
                    return Err(invalid("動画検査結果が不正です"));
                };
                (p.byte_length, AssetProbe::Video(probe))
            }
            AssetProbe::Image(p) => (
                p.byte_length,
                AssetProbe::Image(
                    sungeki_assets::probe_image(Path::new(path), Default::default())
                        .map_err(invalid)?,
                ),
            ),
            AssetProbe::Psd(p) => {
                let database = shared.lock().map_err(invalid)?.review_database_path.clone();
                let SessionResponse::PsdProbed { probe } =
                    crate::psd_probe::execute(&database, path.clone())?
                else {
                    return Err(invalid("PSD検査結果が不正です"));
                };
                (p.byte_length, AssetProbe::Psd(probe))
            }
            AssetProbe::Audio(p) => {
                let SessionResponse::AudioProbed { probe } =
                    crate::audio_probe::execute(shared, path.clone())?
                else {
                    return Err(invalid("音声検査結果が不正です"));
                };
                (p.byte_length, AssetProbe::Audio(probe))
            }
        };
        if actual != asset.probe || asset.content_hash.as_deref() != Some(actual.sha256()) {
            return Err(ApiError::new(
                ErrorCode::ExternalModification,
                "素材の実体と登録情報が一致しません",
            ));
        }
        // Recheck after probing too, including final-symlink and path replacement.
        sungeki_storage::verify_output(Path::new(path), actual.sha256(), length, &mut stop)
            .map_err(invalid)?;
    }
    if stop() {
        return Err(ApiError::new(
            ErrorCode::Cancelled,
            "素材検査を中断しました",
        ));
    }
    Ok(recipe.assets.len())
}
