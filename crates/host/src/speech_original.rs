//! Bounded revalidation of a generated original. Returns owned verified bytes,
//! never a promise that reopening a mutable path later will return the same file.
use crate::speech_state::Artifact;
use std::{
    fs::{self, Metadata, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
use sungeki_media::{ExpectedAudio, MAX_WAV_BYTES, VerifiedWav, verify_wav};
use sungeki_protocol::{ApiError, ErrorCode};

pub(crate) fn load(artifact: &Artifact) -> Result<VerifiedWav, ApiError> {
    artifact.validate()?;
    let path = Path::new(&artifact.path);
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(io_error)?;
    let before = file.metadata().map_err(io_error)?;
    if !before.is_file() || before.len() != artifact.byte_len || before.len() > MAX_WAV_BYTES as u64
    {
        return Err(changed("原本の種類またはサイズが変わっています"));
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_WAV_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    let after = file.metadata().map_err(io_error)?;
    let current = fs::symlink_metadata(path).map_err(io_error)?;
    if !current.is_file()
        || stamp(&before) != stamp(&after)
        || stamp(&after) != stamp(&current)
        || bytes.len() as u64 != artifact.byte_len
    {
        return Err(changed("読込中に原本が変更・置換されました"));
    }
    let audio = verify_wav(
        bytes,
        ExpectedAudio {
            sample_rate: artifact.sample_rate,
            channels: artifact.channels,
        },
    )
    .map_err(|_| changed("原本の音声形式または内容が変わっています"))?;
    let info = audio.info();
    if info.content_hash != artifact.sha256
        || info.duration_us != artifact.duration_us
        || info.frames != artifact.frames
    {
        return Err(changed("原本のhashまたは実尺が生成時の記録と一致しません"));
    }
    Ok(audio)
}
fn stamp(metadata: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}
fn changed(message: &str) -> ApiError {
    ApiError::new(ErrorCode::ExternalModification, message)
}
fn io_error(error: std::io::Error) -> ApiError {
    if error.kind() == std::io::ErrorKind::NotFound {
        ApiError::new(ErrorCode::AssetMissing, "生成した音声原本が見つかりません")
    } else if error.raw_os_error() == Some(libc::ELOOP) {
        changed("音声原本がシンボリックリンクに置換されています")
    } else {
        ApiError::io(error)
    }
}
