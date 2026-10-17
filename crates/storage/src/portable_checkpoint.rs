//! Checkpoint transport only. This does not import into the DB or certify asset
//! bytes: the collection worker must copy/verify every historical dependency.
use crate::{DocumentError, checkpoint_validation as check, document_paths, project_hash};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use sungeki_project::{Asset, AssetLocator, AssetProbe, Checkpoint, CheckpointSummary, Project};

const MAX_BYTES: usize = 64 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointOrigin {
    pub checkpoint_id: String,
    pub project_id: String,
    pub content_hash: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableCheckpoint {
    format: String,
    version: u32,
    pub id: String,
    pub name: String,
    pub created_at: u64,
    pub source_revision: u32,
    pub origin: CheckpointOrigin,
    /// Hash of the relative Project, independent of the destination directory.
    pub project_sha256: String,
    pub project: Project,
}
fn invalid() -> DocumentError {
    DocumentError::Invalid("持ち出し用の保存時点が不正です".into())
}
fn hash(project: &Project) -> Result<String, DocumentError> {
    let bytes = serde_json::to_vec(project).map_err(|_| invalid())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn asset_path(asset: &Asset) -> String {
    let extension = match &asset.probe {
        AssetProbe::Psd(_) => "psd",
        AssetProbe::Vrm(p) => match p.inspection.kind {
            sungeki_project::VrmKind::Model => "vrm",
            sungeki_project::VrmKind::Motion => "vrma",
        },
        AssetProbe::Video(p) => match p.metadata.container {
            sungeki_project::VideoContainer::Mov => "mov",
            sungeki_project::VideoContainer::Matroska => "mkv",
        },
        AssetProbe::Audio(p) => match p.encoding {
            Some(sungeki_project::CompressedAudioEncoding::Mp3 { .. }) => "mp3",
            None => "wav",
        },
        AssetProbe::Image(p) => match p.format {
            sungeki_project::ImageFormat::Png => "png",
            sungeki_project::ImageFormat::Jpeg => "jpg",
            sungeki_project::ImageFormat::Webp => "webp",
        },
    };
    format!("assets/{}.{}", asset.probe.sha256(), extension)
}
impl PortableCheckpoint {
    /// IDs are supplied by the durable collection input, never regenerated on retry.
    /// All registered assets are retained, even if no current clip uses them.
    pub fn new(source: &Checkpoint, id: &str, project_id: &str) -> Result<Self, DocumentError> {
        check::validate(source)?;
        if !check::uuid(id)
            || !check::uuid(project_id)
            || id == source.summary.id
            || project_id == source.project.id
        {
            return Err(invalid());
        }
        let mut project = source.project.clone();
        project.id = project_id.into();
        for asset in &mut project.assets {
            asset.locator = AssetLocator::ProjectRelative {
                path: asset_path(asset),
            };
        }
        let result = Self {
            format: "vvm-collection-checkpoint".into(),
            version: 1,
            id: id.into(),
            name: source.summary.name.clone(),
            created_at: source.summary.created_at,
            source_revision: source.summary.source_revision,
            origin: CheckpointOrigin {
                checkpoint_id: source.summary.id.clone(),
                project_id: source.project.id.clone(),
                content_hash: source.summary.content_hash.clone(),
            },
            project_sha256: hash(&project)?,
            project,
        };
        result.validate()?;
        Ok(result)
    }
    fn validate(&self) -> Result<(), DocumentError> {
        if self.format != "vvm-collection-checkpoint"
            || self.version != 1
            || !check::metadata(&self.id, &self.name, self.created_at)
            || !check::uuid(&self.project.id)
            || !check::uuid(&self.origin.checkpoint_id)
            || !check::uuid(&self.origin.project_id)
            || self.id == self.origin.checkpoint_id
            || self.project.id == self.origin.project_id
            || self.origin.content_hash.len() != 64
            || !self
                .origin
                .content_hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || hash(&self.project)? != self.project_sha256
            || self.project.assets.iter().any(|asset| {
                asset.locator
                    != (AssetLocator::ProjectRelative {
                        path: asset_path(asset),
                    })
            })
        {
            return Err(invalid());
        }
        // Runtime validation requires absolute locators; this temporary clone is
        // schema-checked only and must never be used to read files.
        let mut runtime_shape = self.project.clone();
        for asset in &mut runtime_shape.assets {
            asset.locator = AssetLocator::Absolute {
                path: format!("/{}", asset_path(asset)),
            };
        }
        sungeki_project::validate(&runtime_shape).map_err(|_| invalid())?;
        Ok(())
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, DocumentError> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|_| invalid())?;
        if bytes.len() > MAX_BYTES {
            return Err(invalid());
        }
        Ok(bytes)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DocumentError> {
        if bytes.len() > MAX_BYTES {
            return Err(invalid());
        }
        let result: Self = serde_json::from_slice(bytes).map_err(|_| invalid())?;
        result.validate()?;
        Ok(result)
    }
    /// `root` is the collection root, not its checkpoints subdirectory.
    /// Resolving references does not imply that missing/changed originals are usable.
    pub fn resolve(
        &self,
        root: &Path,
        expected_project_id: &str,
    ) -> Result<Checkpoint, DocumentError> {
        self.validate()?;
        if self.project.id != expected_project_id
            || !root.is_absolute()
            || !std::fs::symlink_metadata(root)?.is_dir()
        {
            return Err(invalid());
        }
        let root = root.canonicalize()?;
        let mut project = self.project.clone();
        document_paths::resolve(&mut project, &root)?;
        let checkpoint = Checkpoint {
            summary: CheckpointSummary {
                id: self.id.clone(),
                project_id: project.id.clone(),
                name: self.name.clone(),
                created_at: self.created_at,
                source_revision: self.source_revision,
                content_hash: project_hash(&project)?,
            },
            asset_manifest: check::manifest(&project),
            project,
        };
        check::validate(&checkpoint)?;
        Ok(checkpoint)
    }
}
