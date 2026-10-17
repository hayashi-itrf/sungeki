//! VRM/VRMA original identity only; model parsing and rendering remain separate.
use crate::{ImageReviewError, Issue, Severity};
use std::path::Path;
use sungeki_media::{WaveformCacheError, source_identity};
use sungeki_project::{AssetLocator, AssetProbe, Project};
pub fn review_vrm_sources(
    project: &Project,
    max_bytes: u64,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<Issue>, ImageReviewError> {
    let mut remaining = max_bytes;
    let mut issues = Vec::new();
    for (index, asset) in project.assets.iter().enumerate() {
        if cancelled() {
            return Err(ImageReviewError::Cancelled);
        }
        let AssetProbe::Vrm(probe) = &asset.probe else {
            continue;
        };
        let result = if probe.byte_length > remaining || probe.byte_length > 100 * 1024 * 1024 {
            Some((
                Severity::Warning,
                "VRM_SOURCE_NOT_INSPECTED",
                "VRM/VRMA原本が検査容量の上限を超えるため未検査です。".into(),
            ))
        } else if let AssetLocator::Absolute { path } = &asset.locator {
            remaining -= probe.byte_length;
            match source_identity(Path::new(path), probe.byte_length, &cancelled) {
                Ok((hash, size))
                    if hash == probe.sha256
                        && size == probe.byte_length
                        && asset.content_hash.as_deref() == Some(hash.as_str()) =>
                {
                    None
                }
                Ok(_)
                | Err(WaveformCacheError::SourceChanged)
                | Err(WaveformCacheError::Wav(sungeki_media::WavError::TooLarge)) => Some((
                    Severity::Error,
                    "VRM_SOURCE_CHANGED",
                    "VRM/VRMA原本が登録時から、または読込み中に変更されています。".into(),
                )),
                Err(WaveformCacheError::Cancelled) => return Err(ImageReviewError::Cancelled),
                Err(WaveformCacheError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                    Some((
                        Severity::Error,
                        "VRM_SOURCE_MISSING",
                        "VRM/VRMA原本が見つかりません。".into(),
                    ))
                }
                Err(e) => Some((
                    Severity::Error,
                    "VRM_SOURCE_UNREADABLE",
                    format!("VRM/VRMA原本を検査できません: {e}"),
                )),
            }
        } else {
            Some((
                Severity::Warning,
                "VRM_SOURCE_NOT_INSPECTED",
                "VRM/VRMA原本の絶対パスが未解決のため未検査です。".into(),
            ))
        };
        if let Some((severity, code, message)) = result {
            issues.push(Issue {
                severity,
                code: code.into(),
                field: format!("/assets/{index}"),
                asset_id: Some(asset.id.clone()),
                clip_id: None,
                scene_id: None,
                start_us: None,
                end_us: None,
                message,
            });
        }
    }
    if cancelled() {
        return Err(ImageReviewError::Cancelled);
    }
    Ok(issues)
}
