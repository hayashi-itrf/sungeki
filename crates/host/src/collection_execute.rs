//! Collection use case shared by protocol requests and background workers.
#![allow(dead_code)]
#[path = "collection_api.rs"]
pub(crate) mod api;
#[path = "collection_control.rs"]
pub(crate) mod control;
#[path = "collection_dry_run.rs"]
mod dry_run;
#[path = "collection_errors.rs"]
mod errors;
#[path = "collection_freeze.rs"]
mod freeze;
#[path = "collection_history.rs"]
pub(crate) mod history;
#[path = "collection_import.rs"]
pub(crate) mod import;
#[path = "collection_manifest.rs"]
mod manifest;
#[path = "collection_receipt.rs"]
pub(crate) mod receipt;
#[path = "collection_record.rs"]
pub(crate) mod record;
#[path = "collection_recovery.rs"]
pub(crate) mod recovery;
#[cfg(test)]
#[path = "collection_execute_tests.rs"]
mod tests;
#[path = "collection_worker.rs"]
mod worker;
use std::{
    fs,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use sungeki_assets::{CollectionInput, CollectionLimits, CollectionSourceStatus, plan_collection};
use sungeki_project::{AssetLocator, Project};
use sungeki_storage::{CollectionStage, DocumentError, PublishError};

#[derive(Debug)]
pub(crate) enum Error {
    Api(sungeki_protocol::ApiError),
    Plan(sungeki_assets::CollectionError),
    SourcesNotReady,
    Document(DocumentError),
    Publish(PublishError),
    Io(std::io::Error),
    Invalid(String),
    Limit(String),
}
impl From<PublishError> for Error {
    fn from(e: PublishError) -> Self {
        Self::Publish(e)
    }
}
impl From<DocumentError> for Error {
    fn from(e: DocumentError) -> Self {
        Self::Document(e)
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// A single frozen current snapshot. Checkpoints are deliberately not accepted
/// until their origin/identity and explicit-import semantics are implemented.
/// This does not mutate the editor, document binding or recovery database.
pub(crate) fn execute(
    source: &Project,
    destination: &Path,
    limits: CollectionLimits,
    cancelled: impl Fn() -> bool,
) -> Result<PathBuf, Error> {
    let id = uuid::Uuid::new_v4().to_string();
    execute_recorded(source, destination, limits, &id, cancelled, |_| Ok(()))
        .map(|receipt| receipt.destination)
}

/// The caller must durably store the receipt before this callback returns.
/// Publication never begins if recording fails. A recorded receipt alone is not
/// success: after restart verify the destination bytes before reporting completion.
pub(crate) fn execute_recorded(
    source: &Project,
    destination: &Path,
    limits: CollectionLimits,
    collected_id: &str,
    cancelled: impl Fn() -> bool,
    before_publish: impl FnOnce(&receipt::Receipt) -> Result<(), Error>,
) -> Result<receipt::Receipt, Error> {
    execute_with_history(
        source,
        &[],
        destination,
        limits,
        collected_id,
        cancelled,
        before_publish,
    )
}

/// History must be frozen together with the current snapshot and new IDs.
pub(crate) fn execute_with_history(
    source: &Project,
    history: &[history::FrozenCheckpoint],
    destination: &Path,
    limits: CollectionLimits,
    collected_id: &str,
    cancelled: impl Fn() -> bool,
    before_publish: impl FnOnce(&receipt::Receipt) -> Result<(), Error>,
) -> Result<receipt::Receipt, Error> {
    execute_journaled(
        source,
        history,
        destination,
        limits,
        collected_id,
        cancelled,
        |_| Ok(()),
        before_publish,
    )
}
pub(crate) fn execute_journaled(
    source: &Project,
    history: &[history::FrozenCheckpoint],
    destination: &Path,
    limits: CollectionLimits,
    collected_id: &str,
    cancelled: impl Fn() -> bool,
    stage_created: impl FnOnce(&sungeki_storage::CollectionStageRecord) -> Result<(), Error>,
    before_publish: impl FnOnce(&receipt::Receipt) -> Result<(), Error>,
) -> Result<receipt::Receipt, Error> {
    if uuid::Uuid::parse_str(collected_id)
        .ok()
        .is_none_or(|id| id.to_string() != collected_id)
        || collected_id == source.id
    {
        return Err(Error::Invalid(
            "収集コピーには新しい標準UUIDが必要です".into(),
        ));
    }
    let prepared = history::Prepared::new(source, collected_id, history)?;
    let history_bytes = prepared.byte_length()?;
    let mut inputs = vec![CollectionInput { project: source }];
    inputs.extend(history.iter().map(|item| CollectionInput {
        project: &item.source.project,
    }));
    let plan = plan_collection(&inputs, limits, &cancelled).map_err(Error::Plan)?;
    if !plan.sources_ready() {
        return Err(Error::Api(errors::sources(&plan)));
    }
    let stage = CollectionStage::new(destination)?;
    stage_created(&stage.record()?)?;
    if cancelled() {
        return Err(PublishError::Cancelled.into());
    }
    fs::create_dir(stage.path().join("assets"))?;
    let mut collected = source.clone();
    collected.id = collected_id.into();
    for asset in &mut collected.assets {
        let relative = plan
            .destination_for(0, &asset.id)
            .ok_or_else(|| Error::Invalid("素材の収集先がありません".into()))?;
        asset.locator = AssetLocator::Absolute {
            path: stage
                .path()
                .join(relative)
                .to_str()
                .ok_or_else(|| Error::Invalid("保存先はUTF-8が必要です".into()))?
                .into(),
        };
    }
    let document = stage.path().join("project.vvm.json");
    // Account for metadata as well as asset bytes before starting any large copy.
    let document_estimate = serde_json::to_vec_pretty(&collected)
        .map_err(|e| Error::Invalid(e.to_string()))?
        .len() as u64;
    let mut manifest = manifest::Manifest::new(source, &collected, &plan)?;
    manifest.add_history(prepared.manifest());
    let manifest_estimate = manifest.bytes()?.len() as u64;
    let required = plan
        .estimated_asset_bytes
        .checked_add(history_bytes)
        .and_then(|n| n.checked_add(document_estimate))
        .and_then(|n| n.checked_add(manifest_estimate))
        .and_then(|n| n.checked_add(16 * 1024 * 1024))
        .ok_or_else(|| Error::Invalid("収集容量の上限です".into()))?;
    sungeki_media::ensure_available_space(stage.path(), required)?;
    for file in &plan.files {
        let original = file
            .source_indices
            .iter()
            .map(|i| &plan.sources[*i])
            .find(|s| s.status == CollectionSourceStatus::Verified)
            .and_then(|s| s.path.as_ref())
            .ok_or(Error::SourcesNotReady)?;
        sungeki_storage::copy_collection_asset(
            original,
            &stage.path().join(&file.relative_path),
            &file.sha256,
            file.byte_length,
            &cancelled,
        )?;
    }
    if cancelled() {
        return Err(PublishError::Cancelled.into());
    }
    prepared.write(stage.path(), &cancelled)?;
    let stamp = sungeki_storage::save_document(&document, &collected, None)?;
    manifest.project_sha256 = stamp.hash;
    let bytes = manifest.bytes()?;
    let manifest_path = stage.path().join("manifest.json");
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&manifest_path)?;
    output.write_all(&bytes)?;
    output.sync_all()?;
    drop(output);
    // Reopen exactly the document a normal user will open, and verify the bytes
    // and all copied originals before making the directory visible.
    let reopened = sungeki_storage::load_document(&document)?;
    if reopened.project != collected
        || reopened.stamp.hash != manifest.project_sha256
        || fs::read(&manifest_path)? != bytes
    {
        return Err(PublishError::Changed.into());
    }
    for file in &plan.files {
        sungeki_storage::verify_output(
            &stage.path().join(&file.relative_path),
            &file.sha256,
            file.byte_length,
            &cancelled,
        )?;
    }
    let mut receipt =
        receipt::Receipt::new(destination, collected_id, &document, &manifest_path, &plan)?;
    receipt.files.extend(prepared.files());
    receipt.validate()?;
    before_publish(&receipt)?;
    stage.commit(cancelled)?;
    Ok(receipt)
}
