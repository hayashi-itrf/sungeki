use crate::{ApiError, ErrorCode};
use serde::{Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const MAX_HANDSHAKE_BYTES: usize = 64 * 1024;
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

pub async fn read_frame<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
) -> Result<T, ApiError> {
    read_frame_limited(reader, MAX_FRAME_BYTES).await
}

pub async fn read_frame_limited<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
    limit: usize,
) -> Result<T, ApiError> {
    let length = reader.read_u32().await.map_err(ApiError::io)? as usize;
    if length == 0 || length > limit.min(MAX_FRAME_BYTES) {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "通信フレーム長が不正です",
        ));
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).await.map_err(ApiError::io)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| ApiError::new(ErrorCode::ValidationFailed, error.to_string()))
}

pub async fn write_frame<T: Serialize>(
    writer: &mut (impl AsyncWrite + Unpin),
    value: &T,
) -> Result<(), ApiError> {
    let bytes = serde_json::to_vec(value).map_err(ApiError::io)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "通信フレーム上限を超えました",
        ));
    }
    writer
        .write_u32(bytes.len() as u32)
        .await
        .map_err(ApiError::io)?;
    writer.write_all(&bytes).await.map_err(ApiError::io)?;
    writer.flush().await.map_err(ApiError::io)
}
