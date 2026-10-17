//! Prepare persistent audio copies before publishing a document. Does not mutate
//! Project/history/bindings or delete originals. The save transaction owns rebasing.
use crate::speech_state::Artifact;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
};
use sungeki_project::{Asset, AssetLocator, AssetProbe, AudioRelocation};
use sungeki_protocol::{ApiError, ErrorCode};

#[derive(Debug)]
pub(crate) struct MaterializedAudio {
    pub relocation: AudioRelocation,
    pub destination: PathBuf,
    pub relative_path: String,
}

/// Blocking I/O boundary. Caller supplies the audio inventory for the document
/// and retained history/checkpoints, and a total byte budget. Publication must
/// happen only after ALL copies are prepared. Partial copies are safe to retain.
pub(crate) fn prepare(
    assets: &[Asset],
    document: &Path,
    max_total_bytes: u64,
) -> Result<Vec<MaterializedAudio>, ApiError> {
    let document = sungeki_storage::document_location(document)
        .map_err(crate::session_save::document_error)?;
    let name = document
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".vvm.json"))
        .filter(|n| !n.is_empty())
        .ok_or_else(|| error("作品のファイル名が不正です"))?;
    let folder = format!("{name}.assets");
    let parent = document
        .parent()
        .ok_or_else(|| error("作品の保存先がありません"))?;
    let mut unique = BTreeMap::new();
    let mut total = 0u64;
    // Preflight all metadata and budget before creating directories or copying.
    for asset in assets {
        let AssetProbe::Audio(probe) = &asset.probe else {
            return Err(error("音声素材を指定してください"));
        };
        probe.validate().map_err(|e| error(&e.to_string()))?;
        if asset.content_hash.as_deref() != Some(probe.sha256.as_str()) {
            return Err(error("音声素材のhashが一致しません"));
        }
        if let Some(previous) = unique.insert(probe.sha256.clone(), probe) {
            if previous != probe {
                return Err(error("同じhashの音声情報が矛盾しています"));
            }
        } else {
            total = total
                .checked_add(probe.byte_length)
                .filter(|n| *n <= max_total_bytes)
                .ok_or_else(|| error("音声素材の保存容量上限を超えています"))?;
        }
        source(asset)?;
    }
    if assets.is_empty() {
        return Ok(vec![]);
    }
    let root = parent.join(&folder);
    directory(&root)?;
    let audio_dir = root.join("audio");
    directory(&audio_dir)?;
    let mut output = Vec::with_capacity(assets.len());
    for asset in assets {
        let original = source(asset)?;
        // Revalidate every source even when its hash shares an existing copy.
        let wav = crate::speech_original::load(&original)?;
        let relative_path = format!("{folder}/audio/{}.wav", original.sha256);
        let destination = parent.join(&relative_path);
        let copied = Artifact {
            path: destination
                .to_str()
                .ok_or_else(|| error("音声保存パスがUTF-8ではありません"))?
                .into(),
            ..original
        };
        match fs::symlink_metadata(&destination) {
            Ok(_) => {
                crate::speech_original::load(&copied)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Err(e) = sungeki_media::store_wav_new(&wav, &destination) {
                    if !e.published && e.source.kind() == std::io::ErrorKind::AlreadyExists {
                        crate::speech_original::load(&copied)?;
                    } else {
                        return Err(ApiError::io(e));
                    }
                }
            }
            Err(e) => return Err(ApiError::io(e)),
        }
        crate::speech_original::load(&copied)?;
        fs::File::open(&destination)
            .and_then(|f| f.sync_all())
            .map_err(ApiError::io)?;
        fs::File::open(&audio_dir)
            .and_then(|f| f.sync_all())
            .map_err(ApiError::io)?;
        output.push(MaterializedAudio {
            relocation: AudioRelocation {
                asset_id: asset.id.clone(),
                source: asset.locator.clone(),
                probe: copied.probe(),
                destination: AssetLocator::Absolute { path: copied.path },
            },
            destination,
            relative_path,
        });
    }
    Ok(output)
}
fn source(asset: &Asset) -> Result<Artifact, ApiError> {
    let (AssetLocator::Absolute { path }, AssetProbe::Audio(probe)) =
        (&asset.locator, &asset.probe)
    else {
        return Err(error("音声コピーには解決済みの絶対パスが必要です"));
    };
    let artifact = Artifact {
        path: path.clone(),
        sha256: probe.sha256.clone(),
        duration_us: probe.duration_us.0,
        sample_rate: probe.sample_rate,
        channels: probe.channels,
        frames: probe.frames,
        byte_len: probe.byte_length,
        lip_sync: None,
    };
    artifact.validate()?;
    Ok(artifact)
}
fn directory(path: &Path) -> Result<(), ApiError> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(ApiError::io(e)),
    }
    let metadata = fs::symlink_metadata(path).map_err(ApiError::io)?;
    // SAFETY: geteuid has no arguments or failure mode.
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o022 != 0
    {
        return Err(error(
            "音声素材フォルダーの種類・所有者・書込権限が不正です",
        ));
    }
    fs::File::open(
        path.parent()
            .ok_or_else(|| error("素材フォルダーの親がありません"))?,
    )
    .and_then(|f| f.sync_all())
    .map_err(ApiError::io)?;
    Ok(())
}
fn error(message: &str) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message)
}
