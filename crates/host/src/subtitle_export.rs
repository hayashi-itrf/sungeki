use crate::{render_runtime::Runtime, sessions::Sessions};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{Arc, Mutex, atomic::Ordering},
    time::{Duration, Instant},
};
use sungeki_protocol::*;
use sungeki_storage::PublishError;

struct Active(Runtime);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.wake.notify_waiters();
    }
}

pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: ExportSubtitlesArgs,
) -> Result<SessionResponse, ApiError> {
    let (extension, format) = match args.format {
        SubtitleFormat::Srt => ("srt", sungeki_render::SubtitleFormat::Srt),
        SubtitleFormat::WebVtt => ("vtt", sungeki_render::SubtitleFormat::WebVtt),
    };
    let path = Path::new(&args.path);
    if !path.is_absolute()
        || !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case(extension))
    {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "字幕の形式に合う拡張子の絶対保存先が必要です",
        ));
    }
    let (project, runtime) = {
        let sessions = shared.lock().map_err(crate::render_record::invalid)?;
        let (_, current) = sessions.load(&args.session_id)?;
        if current.history().revision() != args.expected_revision {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "作品が更新されています。取得し直してください",
            ));
        }
        (
            current.history().project().clone(),
            sessions.render_runtime.clone(),
        )
    };
    // Register with shutdown before work; do not hold the editor lock during file I/O.
    let active = {
        let flags = runtime
            .flags
            .lock()
            .map_err(crate::render_record::invalid)?;
        if runtime.stopping.load(Ordering::SeqCst) {
            return Err(ApiError::new(ErrorCode::Cancelled, "hostは終了処理中です"));
        }
        runtime.active.fetch_add(1, Ordering::SeqCst);
        drop(flags);
        Active(runtime)
    };
    let content_hash =
        sungeki_storage::project_hash(&project).map_err(crate::render_record::invalid)?;
    let text = sungeki_render::subtitle_document(&project, format, args.include_text).map_err(
        |error| {
            ApiError::new(
                ErrorCode::ValidationFailed,
                format!("字幕を生成できません: {error:?}"),
            )
        },
    )?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let result = sungeki_storage::publish_subtitles(path, &text, || {
        active.0.stopping.load(Ordering::SeqCst) || Instant::now() >= deadline
    })
    .map_err(publication_error)?;
    // Publication already happened. If state cannot be rechecked, conservatively mark stale.
    let stale = shared
        .lock()
        .ok()
        .and_then(|sessions| {
            sessions.load(&args.session_id).ok().map(|(_, state)| {
                state.history().revision() != args.expected_revision
                    || sungeki_storage::project_hash(state.history().project())
                        .ok()
                        .as_ref()
                        != Some(&content_hash)
            })
        })
        .unwrap_or(true);
    Ok(SessionResponse::SubtitlesExported {
        publication: SubtitlePublication {
            session_id: args.session_id,
            source_revision: args.expected_revision,
            content_hash,
            path: result.path.to_string_lossy().into_owned(),
            format: args.format,
            include_text: args.include_text,
            sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
            byte_length: text.len() as u64,
            already_present: result.already_present,
            stale,
        },
    })
}

fn publication_error(error: PublishError) -> ApiError {
    match error {
        PublishError::InvalidRange
        | PublishError::InvalidDestination
        | PublishError::InvalidFingerprint => {
            ApiError::new(ErrorCode::ValidationFailed, "有効な字幕保存先が必要です")
        }
        PublishError::Changed => ApiError::new(
            ErrorCode::ExternalModification,
            "保存中に出力先が変更されました",
        ),
        PublishError::OutputExists => ApiError::new(
            ErrorCode::OutputExists,
            "同名ファイルを上書きできません。別の保存先を選んでください",
        ),
        PublishError::Cancelled => ApiError::new(
            ErrorCode::Cancelled,
            "字幕の保存は確定前に停止しました。再試行できます",
        ),
        PublishError::Io(error) => ApiError::io(error),
        PublishError::DurabilityUncertain { path, source } => {
            let mut error = ApiError::new(
                ErrorCode::IoError,
                format!(
                    "字幕は存在しますが保存の同期を確認できません。同じ要求で再確認してください: {source}"
                ),
            );
            error.retryable = true;
            error
                .details
                .insert("path".into(), path.to_string_lossy().into_owned());
            error
                .details
                .insert("publication".into(), "durability_uncertain".into());
            error
        }
    }
}
