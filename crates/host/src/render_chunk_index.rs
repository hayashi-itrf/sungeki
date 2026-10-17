use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use sungeki_protocol::{ApiError, ErrorCode};
pub(crate) fn create(
    path: &Path,
    length: u64,
    expected: &str,
    cancelled: &AtomicBool,
) -> Result<Vec<String>, ApiError> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(ApiError::io)?;
    let metadata = file.metadata().map_err(ApiError::io)?;
    if !metadata.is_file() || length == 0 || metadata.len() != length {
        return Err(ApiError::new(
            ErrorCode::ExternalModification,
            "動画原本の寸法が変わっています",
        ));
    }
    let mut remaining = length;
    let mut whole = Sha256::new();
    let mut chunks = Vec::new();
    let mut buffer = vec![0; sungeki_render::AUDIO_PREVIEW_CHUNK];
    while remaining > 0 {
        if cancelled.load(Ordering::SeqCst) {
            return Err(ApiError::new(
                ErrorCode::Cancelled,
                "動画の再生準備を取り消しました",
            ));
        }
        let count = remaining.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..count])
            .map_err(ApiError::io)?;
        whole.update(&buffer[..count]);
        chunks.push(format!("{:x}", Sha256::digest(&buffer[..count])));
        remaining -= count as u64;
    }
    if format!("{:x}", whole.finalize()) != expected
        || file.metadata().map_err(ApiError::io)?.len() != length
    {
        return Err(ApiError::new(
            ErrorCode::ExternalModification,
            "動画原本が変更されています",
        ));
    }
    Ok(chunks)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunk_index_checks_whole_original_and_cancellation() {
        let fixture = crate::speech_journal_tests::Fixture::new();
        let path = fixture.0.join("output.mp4");
        let bytes = vec![37; sungeki_render::AUDIO_PREVIEW_CHUNK + 19];
        fs::write(&path, &bytes).unwrap();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let chunks = create(&path, bytes.len() as u64, &hash, &AtomicBool::new(false)).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(
            chunks[1],
            format!("{:x}", Sha256::digest(&bytes[bytes.len() - 19..]))
        );
        assert_eq!(
            create(&path, bytes.len() as u64, &hash, &AtomicBool::new(true))
                .unwrap_err()
                .code,
            ErrorCode::Cancelled
        );
        assert_eq!(
            create(
                &path,
                bytes.len() as u64,
                &"0".repeat(64),
                &AtomicBool::new(false)
            )
            .unwrap_err()
            .code,
            ErrorCode::ExternalModification
        );
    }
}
