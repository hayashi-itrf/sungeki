use crate::{
    loudness_api::record,
    loudness_record::{decode, invalid, save},
    media_job_runtime::Runtime,
    sessions::Sessions,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
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
            eprintln!("loudness job {id}: {error}");
        }
    });
}
async fn run(
    shared: Arc<Mutex<Sessions>>,
    id: &str,
    flag: Arc<AtomicBool>,
) -> Result<(), ApiError> {
    let input = {
        let mut s = shared.lock().map_err(invalid)?;
        let row = record(&s, id)?;
        let (input, mut state) = decode(&row)?;
        let mut life = state.lifecycle()?;
        if flag.load(Ordering::SeqCst) || state.phase == JobPhase::Cancelling {
            life.request_cancel();
            life.acknowledge_cancel().map_err(invalid)?;
            state.phase = life.phase();
            state.progress = life.progress().cloned();
            return save(&mut s, &row, &state);
        }
        life.start().map_err(invalid)?;
        life.update_progress(Progress {
            stage: "元音声を検証し音量を測定中".into(),
            fraction: None,
        })
        .map_err(invalid)?;
        state.phase = life.phase();
        state.progress = life.progress().cloned();
        save(&mut s, &row, &state)?;
        input
    };
    let cancel = flag.clone();
    let output = tokio::task::spawn_blocking(move || {
        use sungeki_media::*;
        let runtime = AudioDecodeRuntime {
            ffmpeg: &input.ffmpeg,
            root: &input.work_root,
            timeout: Duration::from_secs(60),
        };
        let mut analysis = if input.probe.encoding.is_some() {
            let audio =
                decode_mp3_original_file(&input.source_path, &input.probe, &runtime, &cancel)
                    .map_err(crate::audio_decode::error)?;
            measure_decoded_mp3_loudness(
                &audio,
                &input.ffmpeg,
                &input.work_root,
                Duration::from_secs(60),
                &cancel,
            )
        } else {
            let audio = decode_audio_original(&input.source_path, &input.probe, None, &cancel)
                .map_err(crate::audio_decode::error)?;
            measure_loudness(
                &audio,
                &input.ffmpeg,
                &input.work_root,
                Duration::from_secs(60),
                &cancel,
            )
        }
        .map_err(invalid)?;
        analysis.source_sha256 = input.probe.sha256.clone();
        Ok(analysis)
    })
    .await
    .map_err(invalid)
    .and_then(|v| v);
    let mut s = shared.lock().map_err(invalid)?;
    let row = record(&s, id)?;
    let (_, mut state) = decode(&row)?;
    let mut life = state.lifecycle()?;
    if flag.load(Ordering::SeqCst) || state.phase == JobPhase::Cancelling {
        life.request_cancel();
        life.acknowledge_cancel().map_err(invalid)?;
        if let Err(error) = output {
            state.error = Some(error);
        }
    } else {
        if output.is_ok() {
            life.update_progress(Progress {
                stage: "音量測定完了".into(),
                fraction: Some(1.),
            })
            .map_err(invalid)?;
        }
        life.finish(output.is_ok()).map_err(invalid)?;
        match output {
            Ok(output) => state.output = Some(output),
            Err(error) => state.error = Some(error),
        }
    }
    state.phase = life.phase();
    state.progress = life.progress().cloned();
    save(&mut s, &row, &state)
}
