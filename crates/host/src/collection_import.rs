//! Bundle preflight only. No session or checkpoint is written until this succeeds.
use super::{Error, manifest::Manifest};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, os::unix::fs::MetadataExt, path::Path};
use sungeki_assets::{CollectionInput, CollectionLimits, plan_collection};
use sungeki_project::{AssetLocator, Project};
use sungeki_storage::{
    LoadedDocument, PortableCheckpoint, PreparedCheckpointImport, PublishError,
    read_collection_metadata,
};

pub(crate) struct VerifiedImport {
    pub document: LoadedDocument,
    pub checkpoints: Vec<PreparedCheckpointImport>,
}
fn invalid() -> Error {
    Error::Invalid("収集manifestと文書・保存時点の内容が一致しません".into())
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn directory(path: &Path) -> Result<fs::Metadata, Error> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_dir() {
        return Err(PublishError::Changed.into());
    }
    Ok(meta)
}
pub(crate) fn verify(path: &Path, cancelled: impl Fn() -> bool) -> Result<VerifiedImport, Error> {
    if cancelled() {
        return Err(PublishError::Cancelled.into());
    }
    let document = sungeki_storage::load_document(path)?;
    if document.path.file_name().and_then(|s| s.to_str()) != Some("project.vvm.json") {
        return Err(invalid());
    }
    let root = document.path.parent().ok_or_else(invalid)?;
    let mut dirs = vec![(root.to_owned(), directory(root)?)];
    let assets_dir = root.join("assets");
    dirs.push((assets_dir.clone(), directory(&assets_dir)?));
    let manifest_bytes = read_collection_metadata(&root.join("manifest.json"), &cancelled)?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes).map_err(|_| invalid())?;
    if manifest.format != "vvm-collection"
        || manifest.version != 1
        || manifest.project_path != "project.vvm.json"
        || manifest.collected_project_id != document.project.id
        || manifest.source_project_id == document.project.id
        || uuid::Uuid::parse_str(&manifest.source_project_id).is_err()
        || manifest.source_project_sha256.len() != 64
        || !manifest
            .source_project_sha256
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || !manifest.characters_match(&document.project)
        || manifest.project_sha256 != document.stamp.hash
        || manifest.checkpoints.len() > 127
        || manifest.checkpoints_included != !manifest.checkpoints.is_empty()
    {
        return Err(invalid());
    }
    let raw_bytes = read_collection_metadata(&document.path, &cancelled)?;
    let raw: Project = serde_json::from_slice(&raw_bytes).map_err(|_| invalid())?;
    if sha(&raw_bytes) != manifest.project_sha256
        || raw
            .assets
            .iter()
            .any(|a| !matches!(a.locator, AssetLocator::ProjectRelative { .. }))
    {
        return Err(invalid());
    }
    let mut portable = Vec::new();
    let mut resolved = Vec::new();
    let mut ids = HashSet::new();
    let mut total = 0u64;
    if !manifest.checkpoints.is_empty() {
        let dir = root.join("checkpoints");
        dirs.push((dir.clone(), directory(&dir)?));
    }
    for entry in &manifest.checkpoints {
        if uuid::Uuid::parse_str(&entry.id).is_err()
            || entry.path != format!("checkpoints/{}.json", entry.id)
            || !ids.insert(&entry.id)
            || entry.byte_length > 64 * 1024 * 1024
        {
            return Err(invalid());
        }
        total = total
            .checked_add(entry.byte_length)
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(invalid)?;
        let bytes = read_collection_metadata(&root.join(&entry.path), &cancelled)?;
        if bytes.len() as u64 != entry.byte_length || sha(&bytes) != entry.sha256 {
            return Err(PublishError::Changed.into());
        }
        let p = PortableCheckpoint::from_bytes(&bytes)?;
        if p.id != entry.id
            || p.origin != entry.origin
            || p.project_sha256 != entry.project_sha256
            || p.origin.project_id != manifest.source_project_id
            || entry.credits != super::history::credits(&p.project)
            || entry.voice_requirements
                != p.project
                    .characters
                    .iter()
                    .map(|c| c.voice.clone())
                    .collect::<Vec<_>>()
        {
            return Err(invalid());
        }
        resolved.push(p.resolve(root, &document.project.id)?);
        portable.push(p);
    }
    let mut inputs = vec![CollectionInput {
        project: &document.project,
    }];
    inputs.extend(resolved.iter().map(|c| CollectionInput {
        project: &c.project,
    }));
    let plan =
        plan_collection(&inputs, CollectionLimits::default(), &cancelled).map_err(Error::Plan)?;
    if !plan.sources_ready() {
        return Err(Error::Api(super::errors::sources(&plan)));
    }
    for file in &plan.files {
        for index in &file.source_indices {
            if plan.sources[*index].path.as_ref() != Some(&root.join(&file.relative_path)) {
                return Err(invalid());
            }
        }
    }
    if manifest.assets.len() != document.project.assets.len() {
        return Err(invalid());
    }
    for (item, asset) in manifest.assets.iter().zip(&document.project.assets) {
        if item.asset_id != asset.id
            || item.sha256 != asset.probe.sha256()
            || item.name != asset.name
            || item.credit != asset.credit
            || item.license_status != "not_verified"
            || plan.destination_for(0, &asset.id) != Some(item.path.as_str())
            || !plan
                .files
                .iter()
                .any(|f| f.relative_path == item.path && f.byte_length == item.byte_length)
        {
            return Err(invalid());
        }
    }
    let checkpoints = portable
        .iter()
        .map(|p| PreparedCheckpointImport::prepare(p, root, &document.project.id))
        .collect::<Result<Vec<_>, _>>()?;
    for entry in &manifest.checkpoints {
        sungeki_storage::verify_output(
            &root.join(&entry.path),
            &entry.sha256,
            entry.byte_length,
            &cancelled,
        )?;
    }
    // Detect document/manifest/directory replacement over the potentially long scan.
    if read_collection_metadata(&root.join("manifest.json"), &cancelled)? != manifest_bytes
        || sungeki_storage::load_document(&document.path)?.stamp != document.stamp
    {
        return Err(PublishError::Changed.into());
    }
    for (path, before) in dirs {
        let after = directory(&path)?;
        if before.dev() != after.dev() || before.ino() != after.ino() {
            return Err(PublishError::Changed.into());
        }
    }
    if cancelled() {
        return Err(PublishError::Cancelled.into());
    }
    Ok(VerifiedImport {
        document,
        checkpoints,
    })
}
