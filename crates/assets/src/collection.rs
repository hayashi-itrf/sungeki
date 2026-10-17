//! Read-only collection planning. Snapshot assets (including unused registered
//! material) are preserved; current documents and source files are never edited.
use crate::{AssetRoots, resolve};
use std::{collections::BTreeMap, path::PathBuf};
use sungeki_project::{AssetProbe, Project};

pub struct CollectionInput<'a> {
    pub project: &'a Project,
}
#[derive(Debug, Clone, Copy)]
pub struct CollectionLimits {
    pub max_file_bytes: u64,
    pub max_scan_bytes: u64,
}
impl Default for CollectionLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 4 * 1024 * 1024 * 1024,
            max_scan_bytes: 16 * 1024 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectionSourceStatus {
    Verified,
    Missing,
    Changed,
    Unresolved(String),
    Unreadable(String),
    NotInspected,
}
#[derive(Debug)]
pub struct CollectionSource {
    pub document_index: usize,
    pub asset_id: String,
    pub path: Option<PathBuf>,
    pub status: CollectionSourceStatus,
}
#[derive(Debug)]
pub struct CollectionFile {
    pub sha256: String,
    pub byte_length: u64,
    pub relative_path: String,
    pub source_indices: Vec<usize>,
    pub conflicting_metadata: bool,
}
#[derive(Debug)]
pub struct CollectionPlan {
    pub files: Vec<CollectionFile>,
    pub sources: Vec<CollectionSource>,
    /// Expected unique asset bytes, not including document/manifest/checkpoint JSON.
    pub estimated_asset_bytes: u64,
}
impl CollectionPlan {
    pub fn sources_ready(&self) -> bool {
        self.files.iter().all(|f| {
            !f.conflicting_metadata
                && f.source_indices
                    .iter()
                    .any(|&i| self.sources[i].status == CollectionSourceStatus::Verified)
        })
    }
    /// Planned destination only; this does not produce or validate a portable document.
    pub fn destination_for(&self, document_index: usize, asset_id: &str) -> Option<&str> {
        let source = self
            .sources
            .iter()
            .position(|s| s.document_index == document_index && s.asset_id == asset_id)?;
        self.files
            .iter()
            .find(|f| f.source_indices.contains(&source))
            .map(|f| f.relative_path.as_str())
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum CollectionError {
    Invalid(String),
    Cancelled,
}

fn bytes(probe: &AssetProbe) -> u64 {
    match probe {
        AssetProbe::Image(p) => p.byte_length,
        AssetProbe::Audio(p) => p.byte_length,
        AssetProbe::Psd(p) => p.byte_length,
        AssetProbe::Video(p) => p.byte_length,
        AssetProbe::Vrm(p) => p.byte_length,
    }
}
fn extension(probe: &AssetProbe) -> &'static str {
    match probe {
        AssetProbe::Psd(_) => "psd",
        AssetProbe::Vrm(p) => match p.inspection.kind {
            sungeki_project::VrmKind::Model => "vrm",
            sungeki_project::VrmKind::Motion => "vrma",
        },
        AssetProbe::Video(p) => match p.metadata.container {
            sungeki_project::VideoContainer::Mov => "mov",
            sungeki_project::VideoContainer::Matroska => "mkv",
        },
        AssetProbe::Audio(p) => match &p.encoding {
            Some(sungeki_project::CompressedAudioEncoding::Mp3 { .. }) => "mp3",
            None => "wav",
        },
        AssetProbe::Image(p) => match p.format {
            sungeki_project::ImageFormat::Png => "png",
            sungeki_project::ImageFormat::Jpeg => "jpg",
            sungeki_project::ImageFormat::Webp => "webp",
        },
    }
}
/// Plan currently valid host snapshots (absolute original references only).
/// Verify each original within a
/// bounded read budget, and deduplicate by content hash across all snapshots.
/// Missing duplicates remain visible but another verified exact copy can supply
/// the content. Conflicting registered probe metadata is never silently merged.
pub fn plan_collection(
    inputs: &[CollectionInput<'_>],
    limits: CollectionLimits,
    cancelled: impl Fn() -> bool,
) -> Result<CollectionPlan, CollectionError> {
    if inputs.is_empty()
        || inputs.len() > 128
        || limits.max_file_bytes == 0
        || limits.max_scan_bytes == 0
        || inputs
            .iter()
            .map(|i| i.project.assets.len() as u64)
            .sum::<u64>()
            > 100_000
    {
        return Err(CollectionError::Invalid(
            "収集計画の入力または上限が不正です".into(),
        ));
    }
    let mut plan = CollectionPlan {
        files: vec![],
        sources: vec![],
        estimated_asset_bytes: 0,
    };
    let mut seen: BTreeMap<String, (usize, AssetProbe)> = BTreeMap::new();
    let mut remaining = limits.max_scan_bytes;
    for (document_index, input) in inputs.iter().enumerate() {
        if cancelled() {
            return Err(CollectionError::Cancelled);
        }
        sungeki_project::validate(input.project)
            .map_err(|e| CollectionError::Invalid(e.to_string()))?;
        for asset in &input.project.assets {
            if cancelled() {
                return Err(CollectionError::Cancelled);
            }
            let hash = asset.probe.sha256();
            let length = bytes(&asset.probe);
            let file_index = if let Some((index, probe)) = seen.get(hash) {
                if probe != &asset.probe {
                    plan.files[*index].conflicting_metadata = true;
                }
                *index
            } else {
                let index = plan.files.len();
                plan.estimated_asset_bytes = plan
                    .estimated_asset_bytes
                    .checked_add(length)
                    .ok_or_else(|| {
                        CollectionError::Invalid("素材の合計容量が上限を超えます".into())
                    })?;
                plan.files.push(CollectionFile {
                    sha256: hash.into(),
                    byte_length: length,
                    relative_path: format!("assets/{hash}.{}", extension(&asset.probe)),
                    source_indices: vec![],
                    conflicting_metadata: false,
                });
                seen.insert(hash.into(), (index, asset.probe.clone()));
                index
            };
            let (path, status) = match resolve(&asset.locator, &AssetRoots::default()) {
                Err(e) => (None, CollectionSourceStatus::Unresolved(e.to_string())),
                Ok(path) => {
                    let status = if length > limits.max_file_bytes || length > remaining {
                        CollectionSourceStatus::NotInspected
                    } else {
                        // Reserve the full expected size even for an early read failure.
                        remaining -= length;
                        crate::collection_scan::verify(&path, hash, length, &cancelled)?
                    };
                    (Some(path), status)
                }
            };
            let source_index = plan.sources.len();
            plan.sources.push(CollectionSource {
                document_index,
                asset_id: asset.id.clone(),
                path,
                status,
            });
            plan.files[file_index].source_indices.push(source_index);
        }
    }
    if cancelled() {
        return Err(CollectionError::Cancelled);
    }
    Ok(plan)
}
