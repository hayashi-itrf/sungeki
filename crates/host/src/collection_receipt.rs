//! Trusted host-side publication intent. Do not reconstruct this from an
//! untrusted manifest: it is recorded in the job before publishing the folder.
use super::Error;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use sungeki_assets::CollectionPlan;
use sungeki_storage::PublishError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    pub destination: PathBuf,
    pub collected_project_id: String,
    pub files: Vec<File>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct File {
    pub path: String,
    pub sha256: String,
    pub byte_length: u64,
}
impl Receipt {
    pub(super) fn new(
        destination: &Path,
        id: &str,
        document: &Path,
        manifest: &Path,
        plan: &CollectionPlan,
    ) -> Result<Self, Error> {
        let mut files = Vec::with_capacity(plan.files.len() + 2);
        for (name, path) in [("project.vvm.json", document), ("manifest.json", manifest)] {
            let bytes = fs::read(path)?;
            files.push(File {
                path: name.into(),
                sha256: format!("{:x}", Sha256::digest(&bytes)),
                byte_length: bytes.len() as u64,
            });
        }
        files.extend(plan.files.iter().map(|f| File {
            path: f.relative_path.clone(),
            sha256: f.sha256.clone(),
            byte_length: f.byte_length,
        }));
        let parent = destination
            .parent()
            .ok_or(PublishError::InvalidDestination)?
            .canonicalize()?;
        let destination = parent.join(
            destination
                .file_name()
                .ok_or(PublishError::InvalidDestination)?,
        );
        let receipt = Self {
            destination,
            collected_project_id: id.into(),
            files,
        };
        receipt.validate()?;
        Ok(receipt)
    }
    pub fn validate(&self) -> Result<(), Error> {
        let invalid = || Error::Invalid("収集の公開記録が不正です".into());
        if !self.destination.is_absolute()
            || self.destination.file_name().is_none()
            || uuid::Uuid::parse_str(&self.collected_project_id)
                .ok()
                .is_none_or(|id| id.to_string() != self.collected_project_id)
            || !(2..=100_129).contains(&self.files.len())
        {
            return Err(invalid());
        }
        let mut names = HashSet::new();
        for f in &self.files {
            if !names.insert(&f.path)
                || f.byte_length == 0
                || f.sha256.len() != 64
                || !f
                    .sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            {
                return Err(invalid());
            }
            match f.path.as_str() {
                "project.vvm.json" | "manifest.json" if f.byte_length <= 64 * 1024 * 1024 => {}
                p if [
                    "png", "jpg", "webp", "wav", "mp3", "psd", "mov", "mkv", "vrm", "vrma",
                ]
                .iter()
                .any(|ext| p == format!("assets/{}.{ext}", f.sha256)) => {}
                p if checkpoint_path(p) && f.byte_length <= 64 * 1024 * 1024 => {}
                _ => return Err(invalid()),
            }
        }
        if !names.contains(&"project.vvm.json".to_owned())
            || !names.contains(&"manifest.json".to_owned())
        {
            return Err(invalid());
        }
        Ok(())
    }
    /// Read-only confirmation. Never follow a replaced collection/assets symlink,
    /// delete user files, or rely on access to the original source tree.
    pub fn verify(&self, cancelled: impl Fn() -> bool) -> Result<(), Error> {
        use std::os::unix::fs::MetadataExt;
        self.validate()?;
        let root = fs::symlink_metadata(&self.destination)?;
        let assets_path = self.destination.join("assets");
        let assets = fs::symlink_metadata(&assets_path)?;
        if !root.is_dir() || !assets.is_dir() {
            return Err(PublishError::Changed.into());
        }
        let history_path = self.destination.join("checkpoints");
        let history = if self.files.iter().any(|f| checkpoint_path(&f.path)) {
            let meta = fs::symlink_metadata(&history_path)?;
            if !meta.is_dir() {
                return Err(PublishError::Changed.into());
            }
            Some(meta)
        } else {
            None
        };
        for f in &self.files {
            sungeki_storage::verify_output(
                &self.destination.join(&f.path),
                &f.sha256,
                f.byte_length,
                &cancelled,
            )?;
        }
        let loaded = sungeki_storage::load_document(&self.destination.join("project.vvm.json"))?;
        if loaded.project.id != self.collected_project_id
            || self
                .files
                .iter()
                .find(|f| f.path == "project.vvm.json")
                .is_none_or(|f| f.sha256 != loaded.stamp.hash)
        {
            return Err(PublishError::Changed.into());
        }
        let mut directories = vec![(&self.destination, root), (&assets_path, assets)];
        if let Some(meta) = history {
            directories.push((&history_path, meta));
        }
        for (path, before) in directories {
            let after = fs::symlink_metadata(path)?;
            if !after.is_dir() || before.dev() != after.dev() || before.ino() != after.ino() {
                return Err(PublishError::Changed.into());
            }
        }
        if cancelled() {
            return Err(PublishError::Cancelled.into());
        }
        Ok(())
    }
}

fn checkpoint_path(path: &str) -> bool {
    path.strip_prefix("checkpoints/")
        .and_then(|p| p.strip_suffix(".json"))
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == id))
}

#[cfg(test)]
#[path = "collection_receipt_tests.rs"]
mod tests;
