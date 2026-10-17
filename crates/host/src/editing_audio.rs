//! Bounded, verified original reads for the editor. No render job.
#[path = "editing_audio_decode.rs"]
mod decode;
use crate::sessions::Sessions;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{Arc, Mutex},
};
use sungeki_project::{AssetLocator, AssetProbe, ClipContent};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse};
struct Cached {
    key: String,
    stamp: [i128; 7],
    bytes: Vec<u8>,
}
static CACHE: Mutex<Option<Cached>> = Mutex::new(None);
const LIMIT: u64 = 128 * 1024 * 1024;
fn invalid(message: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message.to_string())
}
fn stamp(path: &Path) -> Result<[i128; 7], ApiError> {
    let m = fs::symlink_metadata(path).map_err(ApiError::io)?;
    if !m.is_file() || m.len() > LIMIT {
        return Err(invalid(
            "編集再生の音声は128 MiB以内の通常ファイルにしてください",
        ));
    }
    Ok([
        m.dev().into(),
        m.ino().into(),
        m.len().into(),
        m.mtime().into(),
        m.mtime_nsec().into(),
        m.ctime().into(),
        m.ctime_nsec().into(),
    ])
}
pub fn read(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    revision: u32,
    clip_id: String,
    offset: u32,
    length: u32,
) -> Result<SessionResponse, ApiError> {
    if length == 0 || length > 1024 * 1024 {
        return Err(invalid("音声の読込範囲が不正です"));
    }
    let (asset, stream, cancel, database) = {
        let sessions = shared.lock().map_err(ApiError::io)?;
        let (_, state) = sessions.load(&session_id)?;
        if state.history().revision() != revision {
            return Err(ApiError::new(ErrorCode::Conflict, "作品が更新されています"));
        }
        let project = state.history().project();
        let clip = project
            .clips
            .iter()
            .find(|c| c.id == clip_id)
            .ok_or_else(|| invalid("クリップがありません"))?;
        let (id, stream) = match &clip.content {
            ClipContent::Audio { asset_id, .. } => (asset_id, None),
            ClipContent::Video { asset_id, playback } => (asset_id, playback.audio_stream),
            ClipContent::Speech {
                audio: Some(audio), ..
            } => {
                crate::render_api::check_speech(project, &clip_id).map_err(invalid)?;
                (&audio.asset_id, None)
            }
            _ => return Err(invalid("再生できる合成音声または音声素材がありません")),
        };
        (
            project
                .assets
                .iter()
                .find(|a| &a.id == id)
                .cloned()
                .ok_or_else(|| invalid("音声素材がありません"))?,
            stream,
            sessions.render_runtime.stopping.clone(),
            sessions.review_database_path.clone(),
        )
    };
    let (hash, length_bytes) = match &asset.probe {
        AssetProbe::Audio(p) => (&p.sha256, p.byte_length),
        AssetProbe::Video(p) => (&p.sha256, p.byte_length),
        _ => return Err(invalid("音声素材ではありません")),
    };
    let AssetLocator::Absolute { path } = &asset.locator else {
        return Err(invalid("音声原本の絶対パスが必要です"));
    };
    let path = Path::new(path);
    let before = stamp(path)?;
    let key = format!("{}:{}:{}:{stream:?}", path.display(), hash, length_bytes);
    let mut cache = CACHE.lock().map_err(ApiError::io)?;
    if cache
        .as_ref()
        .is_none_or(|c| c.key != key || c.stamp != before)
    {
        *cache = None;
        let bytes = decode::decode(path, &asset.probe, stream, &database, &cancel)?;
        if stamp(path)? != before {
            return Err(ApiError::new(
                ErrorCode::ExternalModification,
                "音声原本が変更されました",
            ));
        }
        *cache = Some(Cached {
            key,
            stamp: before,
            bytes,
        });
    }
    let bytes = &cache.as_ref().unwrap().bytes;
    if offset as usize >= bytes.len() {
        return Err(invalid("音声の読込位置が範囲外です"));
    }
    let end = (offset as usize + length as usize).min(bytes.len());
    Ok(SessionResponse::EditingAudioBytes {
        offset,
        total: bytes.len() as u32,
        base64: STANDARD.encode(&bytes[offset as usize..end]),
    })
}
