use super::{Settings, invalid, load, validate};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
use sungeki_protocol::{ApiError, ErrorCode};
// Caller holds the host Sessions mutex across comparison and publication.
pub(super) fn save(
    database: &Path,
    expected: &Settings,
    settings: Settings,
) -> Result<Settings, ApiError> {
    validate(&settings)?;
    if &load(database)? != expected {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "実行環境設定が更新されています。再読込してください",
        ));
    }
    let bytes = serde_json::to_vec_pretty(&settings).map_err(invalid)?;
    if bytes.len() as u64 > super::LIMIT {
        return Err(invalid("実行環境設定が大きすぎます"));
    }
    let parent = database
        .parent()
        .ok_or_else(|| invalid("設定フォルダーがありません"))?;
    let temporary = parent.join(format!(".runtime-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(ApiError::io)?;
        file.write_all(&bytes).map_err(ApiError::io)?;
        file.sync_all().map_err(ApiError::io)?;
        fs::rename(&temporary, parent.join("runtime.json")).map_err(ApiError::io)?;
        fs::File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(ApiError::io)?;
        Ok(settings)
    })();
    let _ = fs::remove_file(temporary);
    result
}
