//! Permanent synthesized originals, separate from disposable review work.
use crate::speech_state::{Artifact, invalid};
use std::{
    fs,
    path::{Path, PathBuf},
};
use sungeki_protocol::ApiError;
use sungeki_speech::SpeechResponse;

pub(crate) fn root(database: &Path) -> Result<PathBuf, ApiError> {
    let root = location(database)?;
    match private_create(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(invalid(error)),
    }
    check_directory(&root)?;
    fs::File::open(
        root.parent()
            .ok_or_else(|| invalid("保存領域の親がありません"))?,
    )
    .and_then(|f| f.sync_all())
    .map_err(invalid)?;
    Ok(root)
}
/// Resolve the configured permanent location without creating any directories.
pub(crate) fn location(database: &Path) -> Result<PathBuf, ApiError> {
    use sha2::{Digest, Sha256};
    use std::os::unix::ffi::OsStrExt;
    let database = fs::canonicalize(database).map_err(invalid)?;
    let root = if cfg!(test) {
        database.with_extension("speech-artifacts")
    } else {
        let runtime = crate::runtime_root::configured(&database).map_err(invalid)?;
        let profile = format!("{:x}", Sha256::digest(database.as_os_str().as_bytes()));
        runtime.join(format!("speech-artifacts-{profile}"))
    };
    Ok(root)
}
fn private_create(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o700).create(path)
}
pub(crate) fn check_directory(path: &Path) -> Result<(), ApiError> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::symlink_metadata(path).map_err(invalid)?;
    // SAFETY: geteuid has no arguments or failure mode.
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(invalid("音声保存領域の所有者・権限・種類が不正です"));
    }
    Ok(())
}
/// Blocking worker only. Never overwrite/reuse an existing job directory and
/// never delete a published file on an ambiguous DB failure or cancellation.
pub(crate) fn store(
    root: &Path,
    id: &str,
    response: impl Into<SpeechResponse>,
    settings: sungeki_project::LipSyncSettings,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Artifact, ApiError> {
    if uuid::Uuid::parse_str(id)
        .ok()
        .is_none_or(|uuid| uuid.to_string() != id)
    {
        return Err(invalid("音声成果物のジョブIDが不正です"));
    }
    if cancelled.load(std::sync::atomic::Ordering::Acquire) {
        return Err(ApiError::new(
            sungeki_protocol::ErrorCode::Cancelled,
            "音声解析を取り消しました",
        ));
    }
    let verified = response.into().verify().map_err(invalid)?;
    let lip_sync = sungeki_media::analyze_lipsync(&verified, settings, cancelled).map_err(
        |error| match error {
            sungeki_media::LipSyncError::Cancelled => ApiError::new(
                sungeki_protocol::ErrorCode::Cancelled,
                "音声解析を取り消しました",
            ),
            other => invalid(other),
        },
    )?;
    publish(root, id, &verified, lip_sync, cancelled)
}

/// Publish already verified owned bytes, also used when carrying a successful
/// original into a new production. No engine call or source-path reopen.
pub(crate) fn publish(
    root: &Path,
    id: &str,
    verified: &sungeki_media::VerifiedWav,
    lip_sync: sungeki_project::LipSyncAnalysis,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Artifact, ApiError> {
    if uuid::Uuid::parse_str(id)
        .ok()
        .is_none_or(|uuid| uuid.to_string() != id)
    {
        return Err(invalid("音声成果物のジョブIDが不正です"));
    }
    if cancelled.load(std::sync::atomic::Ordering::Acquire) {
        return Err(ApiError::new(
            sungeki_protocol::ErrorCode::Cancelled,
            "音声の保存を取り消しました",
        ));
    }
    check_directory(root)?;
    let directory = root.join(id);
    let destination = directory.join("audio.wav");
    let info = verified.info();
    let artifact = Artifact {
        path: destination
            .to_str()
            .ok_or_else(|| invalid("音声保存パスがUTF-8ではありません"))?
            .into(),
        sha256: info.content_hash.clone(),
        duration_us: info.duration_us,
        sample_rate: info.sample_rate,
        channels: info.channels,
        frames: info.frames,
        byte_len: verified.bytes().len() as u64,
        lip_sync: Some(lip_sync),
    };
    // Validate metadata before creating a destination; malformed lip sync must
    // not leave a published original that cannot be registered.
    artifact.validate()?;
    private_create(&directory).map_err(invalid)?;
    fs::File::open(root)
        .and_then(|f| f.sync_all())
        .map_err(invalid)?;
    sungeki_media::store_wav_new(verified, &destination).map_err(invalid)?;
    crate::speech_original::load(&artifact)?;
    Ok(artifact)
}
