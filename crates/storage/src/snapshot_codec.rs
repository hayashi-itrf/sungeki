//! Internal recovery representation. Public snapshots and named checkpoints stay unchanged.
use crate::StorageError;
use flate2::bufread::ZlibDecoder;
use sha2::{Digest, Sha256};
use std::io::Read;

const MAGIC: &[u8; 8] = b"VVMREC02";
const HEADER: usize = 48;

pub(crate) fn encode(bytes: &[u8]) -> Result<Vec<u8>, StorageError> {
    let mut result = Vec::new();
    result.extend_from_slice(MAGIC);
    result.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    result.extend_from_slice(&Sha256::digest(bytes));
    let params = brotli::enc::BrotliEncoderParams {
        quality: 2,
        lgwin: 22,
        ..Default::default()
    };
    brotli::BrotliCompress(&mut &bytes[..], &mut result, &params)
        .map_err(StorageError::SnapshotEncoding)?;
    Ok(result)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Vec<u8>, StorageError> {
    if bytes.len() < HEADER || &bytes[..8] != MAGIC {
        return Err(StorageError::InvalidSnapshot);
    }
    let size = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let limit = size.checked_add(1).ok_or(StorageError::InvalidSnapshot)?;
    usize::try_from(limit).map_err(|_| StorageError::InvalidSnapshot)?;
    let mut decoder = brotli::Decompressor::new(&bytes[HEADER..], 65536);
    // Never allocate from an untrusted declared length, or decode past that length.
    let mut result = Vec::new();
    decoder
        .by_ref()
        .take(limit)
        .read_to_end(&mut result)
        .map_err(|_| StorageError::InvalidSnapshot)?;
    if result.len() as u64 != size
        || decoder
            .read(&mut [0])
            .map_err(|_| StorageError::InvalidSnapshot)?
            != 0
        || !decoder.get_ref().is_empty()
        || Sha256::digest(&result)[..] != bytes[16..HEADER]
    {
        return Err(StorageError::InvalidSnapshot);
    }
    Ok(result)
}

pub(crate) fn migrate(tx: &rusqlite::Transaction<'_>, was_v17: bool) -> Result<(), StorageError> {
    let ids = tx
        .prepare("SELECT id FROM sessions ORDER BY id")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    // Process one snapshot at a time, in the same atomic schema migration.
    for id in ids {
        let raw: Vec<u8> =
            tx.query_row("SELECT snapshot FROM sessions WHERE id=?1", [&id], |r| {
                r.get(0)
            })?;
        let decoded;
        let bytes = if was_v17 {
            decoded = decode_v17(&raw)?;
            &decoded
        } else {
            &raw
        };
        let compressed = encode(bytes)?;
        tx.execute(
            "UPDATE sessions SET snapshot=?2 WHERE id=?1",
            rusqlite::params![id, compressed],
        )?;
    }
    Ok(())
}

// One-time Sungeki schema migration only; ordinary reads accept the current codec only.
fn decode_v17(bytes: &[u8]) -> Result<Vec<u8>, StorageError> {
    if bytes.len() < HEADER || &bytes[..8] != b"VVMREC01" {
        return Err(StorageError::InvalidSnapshot);
    }
    let size = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let limit = size.checked_add(1).ok_or(StorageError::InvalidSnapshot)?;
    usize::try_from(limit).map_err(|_| StorageError::InvalidSnapshot)?;
    let mut decoder = ZlibDecoder::new(&bytes[HEADER..]);
    let mut result = Vec::new();
    decoder
        .by_ref()
        .take(limit)
        .read_to_end(&mut result)
        .map_err(|_| StorageError::InvalidSnapshot)?;
    if result.len() as u64 != size
        || decoder.total_in() != (bytes.len() - HEADER) as u64
        || Sha256::digest(&result)[..] != bytes[16..HEADER]
    {
        return Err(StorageError::InvalidSnapshot);
    }
    Ok(result)
}
