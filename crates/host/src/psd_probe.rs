use crate::render_record::invalid;
use std::{path::Path, sync::Mutex};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse};

static PROBING: Mutex<()> = Mutex::new(());
pub(crate) fn execute(_database: &Path, path: String) -> Result<SessionResponse, ApiError> {
    let _permit = PROBING
        .try_lock()
        .map_err(|_| ApiError::new(ErrorCode::Conflict, "別のPSDを検査中です"))?;
    let source = Path::new(&path);
    let (sha256, byte_length) =
        sungeki_assets::image_source_hash(source, Default::default()).map_err(invalid)?;
    let bytes = sungeki_raster::frame::verified(source, &sha256).map_err(invalid)?;
    let probe = sungeki_raster::psd::Document::read(&bytes, &sha256)
        .map_err(invalid)?
        .probe;
    probe.validate().map_err(invalid)?;
    if probe.sha256 != sha256 || probe.byte_length != byte_length {
        return Err(invalid("PSD原本の検査結果が一致しません"));
    }
    Ok(SessionResponse::PsdProbed { probe })
}
