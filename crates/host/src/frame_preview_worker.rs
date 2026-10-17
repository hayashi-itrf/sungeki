use crate::{
    frame_preview_record::{decode, invalid, record, save},
    render_runtime::Runtime,
    sessions::Sessions,
};
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use sungeki_project::{JobPhase, Progress};
use sungeki_protocol::{ApiError, FramePreviewOutput};

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
        lifecycle.start().map_err(invalid)?;
        lifecycle
            .update_progress(Progress {
                stage: "原本固定・確認画像を生成中".into(),
                fraction: None,
            })
            .map_err(invalid)?;
        state.update(lifecycle);
        save(&mut sessions, &job, &state)?;
        input
    };
    let cancellation = flag.clone();
    let output = tokio::task::spawn_blocking(move || -> Result<FramePreviewOutput, ApiError> {
        let plan = sungeki_render::FramePlan::new(&input.project, input.frame_index)
            .map_err(crate::render_record::render_error)?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&input.directory)
            .map_err(invalid)?;
        fs::File::open(
            input
                .directory
                .parent()
                .ok_or_else(|| invalid("作業領域の親がありません"))?,
        )
        .and_then(|f| f.sync_all())
        .map_err(invalid)?;
        let artifact = sungeki_render::render_frame_staged(
            &plan,
            &input.runtime,
            &input.directory,
            &cancellation,
        )
        .map_err(crate::render_record::render_error)?;
        let bytes = serde_json::to_vec(&artifact).map_err(invalid)?;
        let mut report = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(input.directory.join("verification.json"))
            .map_err(invalid)?;
        report.write_all(&bytes).map_err(invalid)?;
        report.sync_all().map_err(invalid)?;
        fs::File::open(&input.directory)
            .and_then(|f| f.sync_all())
            .map_err(invalid)?;
        Ok(FramePreviewOutput {
            path: artifact
                .path
                .to_str()
                .ok_or_else(|| invalid("不正なパス"))?
                .into(),
            sha256: artifact.sha256,
            byte_length: artifact.byte_length,
            width: artifact.width,
            height: artifact.height,
            frame_index: artifact.frame_index,
            time_us: artifact.time_us,
            font_sha256: artifact.font_sha256,
            layout_version: artifact.layout_version,
        })
    })
    .await
    .map_err(invalid)
    .and_then(|v| v);
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
                    stage: "確認画像の検証完了".into(),
                    fraction: Some(1.0),
                })
                .map_err(invalid)?;
        }
        lifecycle.finish(output.is_ok()).map_err(invalid)?;
        match output {
            Ok(output) => state.output = Some(output),
            Err(error) => state.error = Some(error),
        }
    }
    state.update(lifecycle);
    save(&mut sessions, &job, &state)
}
