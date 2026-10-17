use crate::{
    frame_preview_record::{decode, invalid, record},
    sessions::Sessions,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use sungeki_protocol::*;
pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: ReadFramePreviewArgs,
) -> Result<SessionResponse, ApiError> {
    let output = {
        let sessions = shared.lock().map_err(invalid)?;
        let (_, state) = decode(&record(&sessions, &args.job_id)?)?;
        state
            .output
            .ok_or_else(|| ApiError::new(ErrorCode::Conflict, "完成した確認画像がありません"))?
    };
    let deadline = Instant::now() + Duration::from_secs(15);
    let bytes = sungeki_storage::read_verified_output(
        Path::new(&output.path),
        &output.sha256,
        output.byte_length,
        args.offset,
        args.length,
        || Instant::now() >= deadline,
    )
    .map_err(read_error)?;
    Ok(SessionResponse::FramePreviewBytes {
        data: FramePreviewBytes {
            offset: args.offset,
            total: output.byte_length,
            base64: STANDARD.encode(bytes),
        },
    })
}

pub(crate) fn read_error(error: sungeki_storage::PublishError) -> ApiError {
    match error {
        sungeki_storage::PublishError::Cancelled => ApiError::new(
            ErrorCode::Cancelled,
            "画像の読込み期限を超えました。もう一度読み込んでください",
        ),
        other => crate::frame_preview_publish::error(other),
    }
}
