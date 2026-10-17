use crate::{
    render_record::{decode, invalid, record},
    render_runtime::Runtime,
    sessions::Sessions,
};
use std::{
    path::Path,
    sync::{Arc, Mutex, atomic::Ordering},
    time::{Duration, Instant},
};
use sungeki_protocol::*;
use sungeki_storage::PublishError;

static COPYING: Mutex<()> = Mutex::new(());

struct Active(Runtime);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.wake.notify_waiters();
    }
}

pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: PublishRenderArgs,
) -> Result<SessionResponse, ApiError> {
    let _permit = COPYING.try_lock().map_err(|_| {
        let mut error = ApiError::new(ErrorCode::Conflict, "別の完成動画を保存中です");
        error.retryable = true;
        error
    })?;
    let (job, input, mut output, stale, runtime) = {
        let sessions = shared.lock().map_err(invalid)?;
        let job = record(&sessions, &args.job_id)?;
        let (input, state) = decode(&job)?;
        if input.purpose != RenderPurpose::Final {
            return Err(ApiError::new(
                ErrorCode::ValidationFailed,
                "編集用プレビューは完成動画として保存できません。正式出力を実行してください",
            ));
        }
        let output = state.output.ok_or_else(|| {
            ApiError::new(ErrorCode::Conflict, "正常に完成した動画だけ保存できます")
        })?;
        if Path::new(&output.path) != input.directory.join("output.mp4") {
            return Err(invalid("完成動画の記録先が不一致です"));
        }
        let (_, current) = sessions.load(&job.session_id)?;
        let stale = current.history().revision() != input.source_revision
            || sungeki_storage::project_hash(current.history().project()).map_err(invalid)?
                != input.content_hash
            || output.layout_version != sungeki_render::CURRENT_LAYOUT_VERSION;
        (job, input, output, stale, sessions.render_runtime.clone())
    };
    // Coordinate registration with shutdown; never hold the editor lock during copying.
    let active = {
        let flags = runtime.flags.lock().map_err(invalid)?;
        if runtime.stopping.load(Ordering::SeqCst) {
            return Err(ApiError::new(ErrorCode::Cancelled, "hostは終了処理中です"));
        }
        runtime.active.fetch_add(1, Ordering::SeqCst);
        drop(flags);
        Active(runtime)
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let published = sungeki_storage::publish_output(
        Path::new(&output.path),
        Path::new(&args.path),
        &output.sha256,
        output.byte_length,
        || active.0.stopping.load(Ordering::SeqCst) || Instant::now() >= deadline,
    )
    .map_err(error)?;
    output.path = published.path.to_string_lossy().into_owned();
    Ok(SessionResponse::RenderPublished {
        publication: RenderPublication {
            job_id: job.id,
            session_id: job.session_id,
            source_revision: input.source_revision,
            content_hash: input.content_hash,
            output,
            already_present: published.already_present,
            stale,
        },
    })
}

fn error(error: PublishError) -> ApiError {
    match error {
        PublishError::InvalidRange
        | PublishError::InvalidDestination
        | PublishError::InvalidFingerprint => ApiError::new(
            ErrorCode::ValidationFailed,
            "保存先には絶対パスのMP4と有効な完成記録が必要です",
        ),
        PublishError::Changed => ApiError::new(
            ErrorCode::ExternalModification,
            "完成動画が出力時から変更されています",
        ),
        PublishError::OutputExists => ApiError::new(
            ErrorCode::OutputExists,
            "同名ファイルを上書きできません。別の保存先を選んでください",
        ),
        PublishError::Cancelled => ApiError::new(
            ErrorCode::Cancelled,
            "保存は確定前に停止しました（期限またはhost終了）。再試行できます",
        ),
        PublishError::Io(error) => ApiError::io(error),
        PublishError::DurabilityUncertain { path, source } => {
            let mut error = ApiError::new(
                ErrorCode::IoError,
                format!(
                    "完成ファイルは存在しますが保存の同期を確認できません。同じ保存先で再確認してください: {source}"
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
