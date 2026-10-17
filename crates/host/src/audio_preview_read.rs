use crate::{
    audio_preview_record::{decode, invalid, record},
    sessions::Sessions,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    os::unix::fs::OpenOptionsExt,
    sync::{Arc, Mutex},
};
use sungeki_protocol::*;

pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: ReadAudioPreviewArgs,
) -> Result<SessionResponse, ApiError> {
    let (input, state) = {
        let sessions = shared.lock().map_err(invalid)?;
        decode(&record(&sessions, &args.job_id)?)?
    };
    let output = state.output.ok_or_else(|| {
        ApiError::new(ErrorCode::ValidationFailed, "完成した確認音声がありません")
    })?;
    let size = sungeki_render::AUDIO_PREVIEW_CHUNK as u64;
    if args.length == 0 || u64::from(args.length) > size || args.offset >= output.byte_length {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "取得範囲はファイル内の1〜256KiBです",
        ));
    }
    // Never accept a client-supplied path; the persisted job owns this one file.
    let path = input.directory.join("output.mp4");
    if path.to_str() != Some(output.path.as_str()) {
        return Err(invalid("成果物パス不一致"));
    }
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(invalid)?;
    let meta = file.metadata().map_err(invalid)?;
    if !meta.is_file() || meta.len() != output.byte_length {
        return Err(invalid("確認音声が変更されています"));
    }
    let end = (args.offset + u64::from(args.length)).min(output.byte_length);
    let mut bytes = Vec::with_capacity(args.length as usize);
    for index in args.offset / size..=(end - 1) / size {
        let start = index * size;
        let count = (output.byte_length - start).min(size) as usize;
        let mut chunk = vec![0; count];
        file.seek(SeekFrom::Start(start)).map_err(invalid)?;
        file.read_exact(&mut chunk).map_err(invalid)?;
        if output.chunk_sha256.get(index as usize) != Some(&format!("{:x}", Sha256::digest(&chunk)))
        {
            return Err(ApiError::new(
                ErrorCode::ExternalModification,
                "確認音声が変更されています。再生成してください",
            ));
        }
        let from = args.offset.saturating_sub(start) as usize;
        let to = (end - start).min(count as u64) as usize;
        bytes.extend_from_slice(&chunk[from..to]);
    }
    Ok(SessionResponse::AudioPreviewBytes {
        data: AudioPreviewBytes {
            offset: args.offset,
            total: output.byte_length,
            base64: STANDARD.encode(bytes),
        },
    })
}
