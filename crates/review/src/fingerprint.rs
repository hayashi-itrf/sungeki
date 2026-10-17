use sha2::{Digest, Sha256};
use std::path::Path;
use sungeki_assets::{ProbeError, ProbeLimits, image_source_hash};
use sungeki_project::{Asset, AssetLocator};

pub(crate) fn fingerprint(mut entries: Vec<(&str, &str)>) -> String {
    entries.sort_by(|a, b| a.0.cmp(b.0));
    let mut hash = Sha256::new();
    // Persistent hash domain: keep stable across product renames.
    hash.update(b"vvm-next-image-review-v1\0");
    for (id, sha256) in entries {
        hash.update((id.len() as u64).to_be_bytes());
        hash.update(id.as_bytes());
        hash.update(sha256.as_bytes());
    }
    format!("{:x}", hash.finalize())
}
/// Observe source bytes independently of editing. This is a bounded sequential
/// scan, not an atomic filesystem snapshot or a guarantee about future writes.
pub fn current_source_fingerprint(
    assets: &[Asset],
    max_total_bytes: u64,
) -> Result<String, ProbeError> {
    let mut remaining = max_total_bytes;
    let mut entries = Vec::with_capacity(assets.len());
    for asset in assets {
        let AssetLocator::Absolute { path } = &asset.locator else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "素材の参照を解決できません",
            )
            .into());
        };
        let limits = ProbeLimits {
            max_file_bytes: remaining.min(ProbeLimits::default().max_file_bytes),
            ..ProbeLimits::default()
        };
        let (hash, bytes) = match &asset.probe {
            sungeki_project::AssetProbe::Image(_) => image_source_hash(Path::new(path), limits)?,
            #[cfg(unix)]
            sungeki_project::AssetProbe::Audio(_)
            | sungeki_project::AssetProbe::Psd(_)
            | sungeki_project::AssetProbe::Vrm(_)
            | sungeki_project::AssetProbe::Video(_) => sungeki_media::source_identity(
                Path::new(path),
                if matches!(asset.probe, sungeki_project::AssetProbe::Video(_)) {
                    remaining.min(4 * 1024 * 1024 * 1024)
                } else if matches!(asset.probe, sungeki_project::AssetProbe::Vrm(_)) {
                    remaining.min(100 * 1024 * 1024)
                } else {
                    limits.max_file_bytes
                },
                || false,
            )
            .map_err(|error| match error {
                sungeki_media::WaveformCacheError::Io(error) => ProbeError::Io(error),
                sungeki_media::WaveformCacheError::SourceChanged => ProbeError::SourceChanged,
                sungeki_media::WaveformCacheError::Wav(sungeki_media::WavError::TooLarge) => {
                    ProbeError::TooLarge
                }
                other => ProbeError::Io(std::io::Error::other(other.to_string())),
            })?,
            #[cfg(not(unix))]
            _ => return Err(ProbeError::UnsupportedFormat),
        };
        remaining -= bytes;
        entries.push((asset.id.as_str(), hash));
    }
    Ok(fingerprint(
        entries
            .iter()
            .map(|(id, hash)| (*id, hash.as_str()))
            .collect(),
    ))
}
