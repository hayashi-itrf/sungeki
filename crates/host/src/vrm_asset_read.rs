//! Registered VRM originals only. Bounded reads outside the document lock.
use crate::sessions::Sessions;
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom},
    os::unix::fs::OpenOptionsExt,
    sync::{Arc, Mutex},
};
use sungeki_project::{AssetLocator, AssetProbe};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse};

pub fn read(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    revision: u32,
    asset_id: String,
    offset: u32,
    length: u32,
) -> Result<SessionResponse, ApiError> {
    let invalid = |message| ApiError::new(ErrorCode::ValidationFailed, message);
    if length == 0 || length > 4 * 1024 * 1024 {
        return Err(invalid("VRM読込範囲が不正です"));
    }
    let asset = {
        let sessions = shared.lock().map_err(ApiError::io)?;
        let (_, state) = sessions.load(&session_id)?;
        if state.history().revision() != revision {
            return Err(ApiError::new(ErrorCode::Conflict, "作品が更新されています"));
        }
        state
            .history()
            .project()
            .assets
            .iter()
            .find(|a| a.id == asset_id)
            .cloned()
            .ok_or_else(|| invalid("VRM素材がありません"))?
    };
    let (AssetProbe::Vrm(probe), AssetLocator::Absolute { path }) = (&asset.probe, &asset.locator)
    else {
        return Err(invalid("登録済みVRM原本を指定してください"));
    };
    probe
        .validate()
        .map_err(|e| ApiError::new(ErrorCode::ValidationFailed, e.to_string()))?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(ApiError::io)?;
    let meta = file.metadata().map_err(ApiError::io)?;
    if !meta.is_file() || meta.len() != probe.byte_length || offset as u64 >= meta.len() {
        return Err(invalid("VRM原本の長さまたは読込位置が不正です"));
    }
    // Verify the exact opened file, streaming with a fixed-size buffer. No 100 MiB cache.
    let mut hash = Sha256::new();
    let mut remaining = probe.byte_length;
    let mut buffer = [0u8; 64 * 1024];
    while remaining > 0 {
        let size = remaining.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..size]).map_err(ApiError::io)?;
        hash.update(&buffer[..size]);
        remaining -= size as u64;
    }
    if format!("{:x}", hash.finalize()) != probe.sha256 {
        return Err(ApiError::new(
            ErrorCode::ExternalModification,
            "VRM原本が変更されています。素材を再登録してください",
        ));
    }
    file.seek(SeekFrom::Start(offset.into()))
        .map_err(ApiError::io)?;
    let size = (meta.len() - offset as u64).min(length as u64) as usize;
    let mut bytes = vec![0; size];
    file.read_exact(&mut bytes).map_err(ApiError::io)?;
    // The client also checks the complete assembled digest before passing any data to GLTFLoader.
    Ok(SessionResponse::VrmAssetBytes {
        offset,
        total: probe.byte_length as u32,
        sha256: probe.sha256.clone(),
        base64: STANDARD.encode(bytes),
    })
}
