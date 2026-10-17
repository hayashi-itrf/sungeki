//! Frozen history packaging; no database lookup after collection starts.
use super::{Error, receipt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, io::Write, os::unix::fs::OpenOptionsExt, path::Path};
use sungeki_project::{Checkpoint, Project, Voice};
use sungeki_storage::{CheckpointOrigin, PortableCheckpoint, PublishError};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrozenCheckpoint {
    pub collected_id: String,
    pub source: Checkpoint,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestEntry {
    pub id: String,
    pub path: String,
    pub sha256: String,
    pub byte_length: u64,
    pub project_sha256: String,
    pub origin: CheckpointOrigin,
    pub credits: Vec<Credit>,
    pub voice_requirements: Vec<Voice>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Credit {
    kind: String,
    id: String,
    name: String,
    credit: String,
    license_status: String,
}
pub(super) fn credits(project: &Project) -> Vec<Credit> {
    project
        .assets
        .iter()
        .map(|a| Credit {
            kind: "asset".into(),
            id: a.id.clone(),
            name: a.name.clone(),
            credit: a.credit.clone(),
            license_status: "not_verified".into(),
        })
        .chain(project.characters.iter().map(|c| Credit {
            kind: "character".into(),
            id: c.id.clone(),
            name: c.name.clone(),
            credit: c.credit.clone(),
            license_status: "not_verified".into(),
        }))
        .collect()
}
pub(super) struct Prepared {
    entries: Vec<(PortableCheckpoint, Vec<u8>)>,
}
impl Prepared {
    pub fn new(source: &Project, id: &str, frozen: &[FrozenCheckpoint]) -> Result<Self, Error> {
        if frozen.len() > 127 {
            return Err(Error::Limit("保存時点は最大127件です".into()));
        }
        let mut old_ids = HashSet::new();
        let mut new_ids = HashSet::new();
        let mut entries = Vec::new();
        let mut total_bytes = 0usize;
        for item in frozen {
            if item.source.project.id != source.id
                || !old_ids.insert(&item.source.summary.id)
                || !new_ids.insert(&item.collected_id)
            {
                return Err(Error::Invalid(
                    "保存時点の所属または識別子が不正です".into(),
                ));
            }
            let portable = PortableCheckpoint::new(&item.source, &item.collected_id, id)?;
            let bytes = portable.to_bytes()?;
            total_bytes = total_bytes
                .checked_add(bytes.len())
                .filter(|total| *total <= 64 * 1024 * 1024)
                .ok_or_else(|| Error::Limit("保存時点の合計容量上限です".into()))?;
            entries.push((portable, bytes));
        }
        if new_ids.iter().any(|id| old_ids.contains(id)) {
            return Err(Error::Invalid(
                "持ち出し先の保存時点IDが元の履歴と重複しています".into(),
            ));
        }
        Ok(Self { entries })
    }
    pub fn files(&self) -> Vec<receipt::File> {
        self.entries
            .iter()
            .map(|(p, bytes)| receipt::File {
                path: format!("checkpoints/{}.json", p.id),
                sha256: format!("{:x}", Sha256::digest(bytes)),
                byte_length: bytes.len() as u64,
            })
            .collect()
    }
    pub fn manifest(&self) -> Vec<ManifestEntry> {
        self.entries
            .iter()
            .zip(self.files())
            .map(|((p, _), f)| ManifestEntry {
                id: p.id.clone(),
                path: f.path,
                sha256: f.sha256,
                byte_length: f.byte_length,
                project_sha256: p.project_sha256.clone(),
                origin: p.origin.clone(),
                credits: credits(&p.project),
                voice_requirements: p
                    .project
                    .characters
                    .iter()
                    .map(|c| c.voice.clone())
                    .collect(),
            })
            .collect()
    }
    pub fn byte_length(&self) -> Result<u64, Error> {
        // Bound aggregate metadata too; per-file limits alone allow multi-GB history.
        let total: u64 = self
            .entries
            .iter()
            .map(|(_, bytes)| bytes.len() as u64)
            .sum();
        if total > 64 * 1024 * 1024 {
            return Err(Error::Limit("保存時点の合計容量上限です".into()));
        }
        Ok(total)
    }
    pub fn write(&self, root: &Path, cancelled: &impl Fn() -> bool) -> Result<(), Error> {
        if self.entries.is_empty() {
            return Ok(());
        }
        fs::create_dir(root.join("checkpoints"))?;
        for ((portable, bytes), file) in self.entries.iter().zip(self.files()) {
            if cancelled() {
                return Err(PublishError::Cancelled.into());
            }
            let path = root.join(file.path);
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)?;
            output.write_all(bytes)?;
            output.sync_all()?;
            drop(output);
            sungeki_storage::verify_output(&path, &file.sha256, file.byte_length, cancelled)?;
            portable.resolve(root, &portable.project.id)?;
        }
        Ok(())
    }
}
