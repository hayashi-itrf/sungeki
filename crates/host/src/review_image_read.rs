use crate::{
    review_jobs::record,
    review_record::{decode, invalid},
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
    args: ReadReviewImageArgs,
) -> Result<SessionResponse, ApiError> {
    let output = {
        let sessions = shared.lock().map_err(invalid)?;
        let (_, state) = decode(&record(&sessions, &args.job_id)?)?;
        let frames = state
            .report
            .and_then(|report| report.representative_frames)
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "代表画像の記録がありません"))?;
        match args.target {
            ReviewImageTarget::Frame { frame_index } => frames
                .frames
                .into_iter()
                .find(|frame| frame.sample.frame_index == frame_index)
                .and_then(|frame| frame.output),
            ReviewImageTarget::ContactSheet { page } => frames
                .contact_sheets
                .into_iter()
                .find(|sheet| sheet.page == page)
                .and_then(|sheet| sheet.output),
        }
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "指定した成功画像がありません"))?
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
    .map_err(crate::frame_preview_read::read_error)?;
    Ok(SessionResponse::ReviewImageBytes {
        data: FramePreviewBytes {
            offset: args.offset,
            total: output.byte_length,
            base64: STANDARD.encode(bytes),
        },
    })
}
