#[path = "review_frames.rs"]
pub(crate) mod frames;
#[path = "review_image_read.rs"]
pub(crate) mod image_read;
use crate::{
    review_jobs::{Runtime, record, save},
    review_record::{decode, invalid},
    sessions::Sessions,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sungeki_project::{JobPhase, Progress};
use sungeki_protocol::{ApiError, ReviewReport};

struct Active(Runtime, String);
impl Drop for Active {
    fn drop(&mut self) {
        self.0
            .cancellations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.1);
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.notify();
    }
}
pub(crate) fn launch(
    shared: Arc<Mutex<Sessions>>,
    runtime: Runtime,
    id: String,
    cancellation: Arc<AtomicBool>,
) {
    tokio::spawn(async move {
        let _active = Active(runtime.clone(), id.clone());
        let Ok(_slot) = runtime.slots.clone().acquire_owned().await else {
            return;
        };
        if let Err(error) = run(shared, &runtime, &id, cancellation).await {
            eprintln!("review job {id}: {error}");
        }
    });
}
async fn run(
    shared: Arc<Mutex<Sessions>>,
    runtime: &Runtime,
    id: &str,
    cancellation: Arc<AtomicBool>,
) -> Result<(), ApiError> {
    let (input, directory) = {
        let mut sessions = shared.lock().map_err(invalid)?;
        let job = record(&sessions, id)?;
        let (input, mut state) = decode(&job)?;
        let mut lifecycle = state.lifecycle()?;
        if lifecycle.phase() == JobPhase::Cancelling || cancellation.load(Ordering::SeqCst) {
            lifecycle.request_cancel();
            lifecycle.acknowledge_cancel().map_err(invalid)?;
            state.update(lifecycle);
            save(&mut sessions, &job, &state)?;
            return Ok(());
        }
        if let Some(parent) = &input.parent_job_id {
            if let Err(error) = crate::build_record::validate_review_parent(
                &sessions,
                parent,
                &job.id,
                &job.session_id,
                input.source_revision,
            )
            .and_then(|hash| {
                if hash == input.content_hash {
                    Ok(())
                } else {
                    Err(invalid("親の保存内容と確認入力が一致しません"))
                }
            }) {
                if error.code == sungeki_protocol::ErrorCode::Cancelled {
                    lifecycle.request_cancel();
                    lifecycle.acknowledge_cancel().map_err(invalid)?;
                } else {
                    lifecycle.start().map_err(invalid)?;
                    lifecycle.finish(false).map_err(invalid)?;
                }
                state.update(lifecycle);
                state.error = Some(error);
                save(&mut sessions, &job, &state)?;
                return Ok(());
            }
        }
        lifecycle.start().map_err(invalid)?;
        lifecycle
            .update_progress(Progress {
                stage: "作品・画像素材・代表画像を確認中".into(),
                fraction: None,
            })
            .map_err(invalid)?;
        state.update(lifecycle);
        save(&mut sessions, &job, &state)?;
        (
            input,
            crate::review_workdir::path(
                sessions.review_directory.as_ref().map_err(Clone::clone)?,
                id,
            )?,
        )
    };
    runtime.notify();
    let report_id = id.to_owned();
    let worker_cancel = cancellation.clone();
    let report = tokio::task::spawn_blocking(move || {
        match sungeki_review::review_project_images(
            &input.project,
            &directory,
            256 * 1024 * 1024,
            || worker_cancel.load(Ordering::SeqCst),
        ) {
            Ok(output) => frames::complete(
                &input,
                &worker_cancel,
                ReviewReport {
                    report_id,
                    source_revision: input.source_revision,
                    content_hash: input.content_hash.clone(),
                    asset_fingerprint: output.asset_fingerprint,
                    diagnostics: output.diagnostics,
                    representative_frames: None,
                },
            ),
            Err(sungeki_review::ImageReviewError::Cancelled) => Ok(None),
            Err(error) => Err(invalid(error)),
        }
    })
    .await
    .map_err(invalid)
    .and_then(|value| value);
    {
        let mut sessions = shared.lock().map_err(invalid)?;
        let job = record(&sessions, id)?;
        let (_, mut state) = decode(&job)?;
        let mut lifecycle = state.lifecycle()?;
        if lifecycle.phase() == JobPhase::Cancelling || cancellation.load(Ordering::SeqCst) {
            lifecycle.request_cancel();
            // Cancelled work cannot publish diagnostic output; cleanup failures stay visible.
            if let Err(error) = report {
                state.error = Some(error);
            }
            lifecycle.acknowledge_cancel().map_err(invalid)?;
        } else {
            match report {
                Ok(Some(report)) => {
                    lifecycle
                        .update_progress(Progress {
                            stage: "確認処理完了（未検査項目あり）".into(),
                            fraction: Some(1.),
                        })
                        .map_err(invalid)?;
                    lifecycle.finish(true).map_err(invalid)?;
                    state.report = Some(report);
                }
                Ok(None) => {
                    lifecycle.finish(false).map_err(invalid)?;
                    state.error = Some(invalid("取消状態が一致しません"));
                }
                Err(error) => {
                    lifecycle.finish(false).map_err(invalid)?;
                    state.error = Some(error);
                }
            }
        }
        state.update(lifecycle);
        save(&mut sessions, &job, &state)?;
    }
    runtime.notify();
    Ok(())
}
