use crate::{
    CheckKind, CheckStatus, Diagnostics, ImageReviewError, review_images, review_structure,
};
use std::path::Path;
use sungeki_project::Project;

pub struct ReviewOutput {
    pub diagnostics: Diagnostics,
    /// Present when every registered image/audio/PSD/video/VRM source was verified within the scan budget.
    /// This identifies the inspected bytes; it does not assert current freshness.
    pub asset_fingerprint: Option<String>,
}

/// Structure and original-image diagnostics. Other checks retain not_run.
/// The private capture directory is removed successfully before publishing output.
pub fn review_project_images(
    project: &Project,
    directory: &Path,
    max_total_bytes: u64,
    cancelled: impl Fn() -> bool,
) -> Result<ReviewOutput, ImageReviewError> {
    if cancelled() {
        return Err(ImageReviewError::Cancelled);
    }
    let structure = review_structure(project);
    if structure.summary.failed != 0 {
        return Ok(ReviewOutput {
            diagnostics: structure,
            asset_fingerprint: None,
        });
    }
    // Reserve one shared budget across media kinds. A failed/omitted scan does
    // not donate its reservation to later kinds: this keeps the upper bound
    // independent of source changes and failures during inspection.
    let mut remaining = max_total_bytes;
    let mut budgets = [0u64; 5];
    for (kind, budget) in budgets.iter_mut().enumerate() {
        let requested = project.assets.iter().fold(0u64, |total, asset| {
            use sungeki_project::AssetProbe;
            let bytes = match (&asset.probe, kind) {
                (AssetProbe::Image(p), 0) => p.byte_length,
                (AssetProbe::Audio(p), 1) => p.byte_length,
                (AssetProbe::Psd(p), 2) => p.byte_length,
                (AssetProbe::Video(p), 3) => p.byte_length,
                (AssetProbe::Vrm(p), 4) => p.byte_length,
                _ => 0,
            };
            total.saturating_add(bytes)
        });
        *budget = requested.min(remaining);
        remaining -= *budget;
    }
    let mut images = review_images(project, directory, budgets[0], &cancelled)?;
    #[cfg(unix)]
    let mut asset_fingerprint = None;
    #[cfg(not(unix))]
    let mut asset_fingerprint = if images.check.status == CheckStatus::Passed {
        Some(crate::fingerprint::fingerprint(
            images
                .originals
                .iter()
                .map(|entry| (entry.asset_id.as_str(), entry.probe.sha256.as_str()))
                .collect(),
        ))
    } else {
        None
    };
    let (mut audio_check, mut audio_issues) = crate::review_audio_timing(project);
    #[cfg(unix)]
    {
        let mut sources = crate::review_audio_sources(project, budgets[1], &cancelled)?;
        if sources.iter().any(|i| i.severity == crate::Severity::Error) {
            audio_check.status = CheckStatus::Failed;
        }
        audio_check.description = "登録尺による発話配置と、容量上限内の音声原本hash/容量・WAV形式を確認。原本の検査省略は指摘欄へ記載。圧縮音声の復号情報・音量・聴き取りは未検査です。重なりは各発話につき最長の先行発話との1件を表示します。検査後の原本変更は再確認が必要です。".into();
        let mut psd = crate::review_psd_sources(project, budgets[2], &cancelled)?;
        if psd.iter().any(|i| i.severity == crate::Severity::Error) {
            images.check.status = CheckStatus::Failed;
        }
        if project
            .assets
            .iter()
            .any(|a| matches!(a.probe, sungeki_project::AssetProbe::Psd(_)))
        {
            images.check.description.push_str(" PSDは原本hash/容量の照合のみです。レイヤーの再解析・全ポーズの描画は未検査です。検査省略や問題は指摘欄に表示します。");
        }
        let mut videos = crate::review_video_sources(project, budgets[3], &cancelled)?;
        if videos.iter().any(|i| i.severity == crate::Severity::Error) {
            images.check.status = CheckStatus::Failed;
        }
        if project
            .assets
            .iter()
            .any(|a| matches!(a.probe, sungeki_project::AssetProbe::Video(_)))
        {
            images.check.description.push_str(" 動画は原本hash/容量の照合のみです。映像・内蔵音声の復号、時刻情報、再生品質は未検査です。検査省略や問題は指摘欄に表示します。");
        }
        let mut vrm = crate::review_vrm_sources(project, budgets[4], &cancelled)?;
        if vrm.iter().any(|i| i.severity == crate::Severity::Error) {
            images.check.status = CheckStatus::Failed;
        } else if !vrm.is_empty() && images.check.status != CheckStatus::Failed {
            images.check.status = CheckStatus::NotRun;
        }
        if project
            .assets
            .iter()
            .any(|a| matches!(a.probe, sungeki_project::AssetProbe::Vrm(_)))
        {
            images.check.description.push_str(" VRM/VRMAは原本hash/容量の照合のみです。モデル・モーションの再解析と描画は未検査です。検査省略や問題は指摘欄に表示します。");
        }
        // Publish a baseline only when every source was actually inspected.
        // Reuse verified identities, never substitute registered hashes for unread sources.
        let supported_bytes = project.assets.iter().try_fold(0u64, |total, asset| {
            let bytes = match &asset.probe {
                sungeki_project::AssetProbe::Image(p) => p.byte_length,
                sungeki_project::AssetProbe::Audio(p) => p.byte_length,
                sungeki_project::AssetProbe::Psd(p) => p.byte_length,
                sungeki_project::AssetProbe::Video(p) => p.byte_length,
                sungeki_project::AssetProbe::Vrm(p) => p.byte_length,
            };
            total.checked_add(bytes)
        });
        if sources.is_empty()
            && psd.is_empty()
            && videos.is_empty()
            && vrm.is_empty()
            && images.issues.is_empty()
            && supported_bytes.is_some_and(|bytes| bytes <= max_total_bytes)
        {
            asset_fingerprint = Some(crate::fingerprint::fingerprint(
                project
                    .assets
                    .iter()
                    .map(|asset| (asset.id.as_str(), asset.probe.sha256()))
                    .collect(),
            ));
        }
        images.issues.append(&mut psd);
        images.issues.append(&mut videos);
        images.issues.append(&mut vrm);
        audio_issues.append(&mut sources);
    }
    #[cfg(not(unix))]
    for (index, asset) in project.assets.iter().enumerate() {
        if matches!(asset.probe, sungeki_project::AssetProbe::Vrm(_)) {
            asset_fingerprint = None;
            if images.check.status != CheckStatus::Failed {
                images.check.status = CheckStatus::NotRun;
            }
            images
                .check
                .description
                .push_str(" VRM/VRMA原本と描画は未検査です。");
            images.issues.push(crate::Issue {
                severity: crate::Severity::Warning,
                code: "VRM_SOURCE_NOT_INSPECTED".into(),
                field: format!("/assets/{index}"),
                asset_id: Some(asset.id.clone()),
                clip_id: None,
                scene_id: None,
                start_us: None,
                end_us: None,
                message: "VRM/VRMAの登録情報のみ保持しています。原本と描画は未検査です。".into(),
            });
        }
    }
    let checks = structure
        .checks
        .into_iter()
        .map(|check| {
            if check.kind == CheckKind::AssetIntegrity {
                images.check.clone()
            } else if check.kind == CheckKind::Audio {
                audio_check.clone()
            } else {
                check
            }
        })
        .collect();
    let mut issues = structure.issues;
    issues.append(&mut images.issues);
    issues.append(&mut audio_issues);
    images.cleanup().map_err(ImageReviewError::Io)?;
    if cancelled() {
        return Err(ImageReviewError::Cancelled);
    }
    Ok(ReviewOutput {
        diagnostics: Diagnostics::new(checks, issues),
        asset_fingerprint,
    })
}
