#[path = "render_execution.rs"]
mod execution;
#[path = "render_progress.rs"]
pub(crate) mod progress;
#[path = "render_work.rs"]
mod work;
use crate::{
    render_record::{decode, invalid, record, save},
    render_runtime::Runtime,
    sessions::Sessions,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sungeki_project::{JobPhase, Progress};
use sungeki_protocol::ApiError;

struct Active(Runtime, String);
impl Drop for Active {
    fn drop(&mut self) {
        self.0
            .flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.1);
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.wake.notify_one();
    }
}
pub(crate) fn launch(
    shared: Arc<Mutex<Sessions>>,
    runtime: Runtime,
    id: String,
    flag: Arc<AtomicBool>,
) {
    tokio::spawn(async move {
        let _active = Active(runtime.clone(), id.clone());
        let Ok(_slot) = runtime.slots.clone().acquire_owned().await else {
            return;
        };
        if let Err(error) = run(shared, &id, flag).await {
            eprintln!("render job {id}: {error}");
        }
    });
}
async fn run(
    shared: Arc<Mutex<Sessions>>,
    id: &str,
    flag: Arc<AtomicBool>,
) -> Result<(), ApiError> {
    let input = {
        let mut sessions = shared.lock().map_err(invalid)?;
        let job = record(&sessions, id)?;
        let (input, mut state) = decode(&job)?;
        let mut lifecycle = state.lifecycle()?;
        if flag.load(Ordering::SeqCst) || state.phase == JobPhase::Cancelling {
            lifecycle.request_cancel();
            lifecycle.acknowledge_cancel().map_err(invalid)?;
            state.update(lifecycle);
            save(&mut sessions, &job, &state)?;
            return Ok(());
        }
        if let Some(parent) = &input.parent_job_id {
            if let Err(error) = crate::build_record::validate_render_parent(
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
                    Err(invalid("親の保存内容と出力入力が一致しません"))
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
                stage: "原本固定・映像音声を出力中".into(),
                fraction: None,
            })
            .map_err(invalid)?;
        state.update(lifecycle);
        save(&mut sessions, &job, &state)?;
        input
    };
    let cancellation = flag.clone();
    let progress_sessions = shared.clone();
    let progress_id = id.to_owned();
    let (output, cleanup) = match tokio::task::spawn_blocking(move || {
        work::run(&input, &cancellation, &mut |update| {
            if let Err(error) = progress::update(&progress_sessions, &progress_id, update) {
                eprintln!("render progress {progress_id}: {error}");
            }
        })
    })
    .await
    {
        Ok(result) => result,
        Err(error) => (Err(invalid(error)), None),
    };
    {
        let mut sessions = shared.lock().map_err(invalid)?;
        let job = record(&sessions, id)?;
        let (_, mut state) = decode(&job)?;
        let mut lifecycle = state.lifecycle()?;
        if state.phase == JobPhase::Cancelling || flag.load(Ordering::SeqCst) {
            lifecycle.request_cancel();
            lifecycle.acknowledge_cancel().map_err(invalid)?;
            if let Err(error) = output {
                state.error = Some(error);
            }
        } else {
            if output.is_ok() {
                lifecycle
                    .update_progress(Progress {
                        stage: "映像・音声の出力検証完了".into(),
                        fraction: Some(1.0),
                    })
                    .map_err(invalid)?;
            }
            lifecycle.finish(output.is_ok()).map_err(invalid)?;
            match output {
                Ok((output, chunks)) => {
                    state.output = Some(output);
                    state.chunk_sha256 = chunks;
                }
                Err(error) => state.error = Some(error),
            }
        }
        state.update(lifecycle);
        save(&mut sessions, &job, &state)?;
    }
    // Never hold the editor/database lock during disk cleanup. A failed cleanup
    // retains files and does not change the durably saved terminal result.
    if let Some(cleanup) = cleanup {
        match tokio::task::spawn_blocking(move || cleanup.remove()).await {
            Ok(Ok(bytes)) => eprintln!("render job {id}: reclaimed {bytes} intermediate bytes"),
            result => eprintln!("render job {id}: intermediate cleanup incomplete: {result:?}"),
        }
    }
    Ok(())
}
