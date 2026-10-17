use crate::sessions::{Sessions, storage_error};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use sungeki_media::{LOUDNESS_VERSION, LoudnessAnalysis, LoudnessValue};
use sungeki_project::{AudioProbe, JobPhase, Progress};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_storage::{JobKind, StoredJob};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub source_revision: u32,
    pub asset_id: String,
    pub source_path: PathBuf,
    pub probe: AudioProbe,
    pub ffmpeg: PathBuf,
    pub work_root: PathBuf,
    pub analysis_version: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub output: Option<LoudnessAnalysis>,
    pub error: Option<ApiError>,
}
impl State {
    pub fn lifecycle(&self) -> Result<sungeki_jobs::Lifecycle, ApiError> {
        sungeki_jobs::Lifecycle::restore(self.phase, self.progress.clone()).map_err(invalid)
    }
}
pub(crate) fn invalid(error: impl std::fmt::Debug) -> ApiError {
    ApiError::new(ErrorCode::IoError, format!("音量測定ジョブ: {error:?}"))
}
pub(crate) fn decode(job: &StoredJob) -> Result<(Input, State), ApiError> {
    if job.kind != JobKind::Loudness {
        return Err(invalid("ジョブ種類の不一致"));
    }
    let input: Input = serde_json::from_slice(&job.input).map_err(invalid)?;
    let state: State = serde_json::from_slice(&job.state).map_err(invalid)?;
    input.probe.validate().map_err(invalid)?;
    state.lifecycle()?;
    if input.asset_id.is_empty()
        || !input.source_path.is_absolute()
        || !input.ffmpeg.is_absolute()
        || !input.work_root.is_absolute()
        || input.analysis_version != LOUDNESS_VERSION
        || (state.phase == JobPhase::Succeeded) != state.output.is_some()
        || (state.phase == JobPhase::Succeeded && state.error.is_some())
    {
        return Err(invalid("入力または成功状態が不一致です"));
    }
    if let Some(output) = &state.output {
        validate_output(&input, output)?;
    }
    Ok((input, state))
}
fn validate_output(input: &Input, output: &LoudnessAnalysis) -> Result<(), ApiError> {
    if output.version != input.analysis_version
        || output.method != "ffmpeg_ebur128_summary_192k"
        || output.source_sha256 != input.probe.sha256
        || output.source_frames != input.probe.frames
        || output.sample_rate != input.probe.sample_rate
        || output.channels != input.probe.channels
        || !output.source_pre_gain
        || output.dual_mono
    {
        return Err(invalid("測定対象と結果が一致しません"));
    }
    let short = input.probe.frames * 1000 < u64::from(input.probe.sample_rate) * 400;
    let valid = match &output.measurement {
        LoudnessValue::Measured {
            integrated_lufs,
            true_peak_db,
            range_lu,
            threshold_lufs,
        } => {
            !short
                && integrated_lufs.is_finite()
                && *integrated_lufs > -70.
                && true_peak_db.is_finite()
                && range_lu.is_finite()
                && *range_lu >= 0.
                && threshold_lufs.is_finite()
        }
        LoudnessValue::NotMeasurable {
            reason,
            true_peak_db,
        } => {
            use sungeki_media::LoudnessUnavailable;
            true_peak_db.is_none_or(f64::is_finite)
                && match reason {
                    LoudnessUnavailable::TooShort => short && true_peak_db.is_none(),
                    LoudnessUnavailable::BelowGateOrSilence => !short,
                }
        }
    };
    if !valid {
        return Err(invalid("測定値が不正です"));
    }
    Ok(())
}
pub(crate) fn save(
    sessions: &mut Sessions,
    job: &StoredJob,
    state: &State,
) -> Result<(), ApiError> {
    let bytes = serde_json::to_vec(state).map_err(invalid)?;
    // Validate the same invariants before commit and after reopening.
    decode(&StoredJob {
        state: bytes.clone(),
        ..job.clone()
    })?;
    sessions
        .store
        .update_job(&job.id, JobKind::Loudness, job.version, &bytes)
        .map_err(storage_error)?;
    sessions.loudness_runtime.notify_loudness_changed();
    Ok(())
}
pub(crate) fn recover(sessions: &mut Sessions) -> Result<(), ApiError> {
    let mut after = None;
    loop {
        let ids = sessions
            .store
            .list_jobs(Some(JobKind::Loudness), None, after.as_deref(), 100)
            .map_err(storage_error)?;
        if ids.is_empty() {
            return Ok(());
        }
        for id in &ids {
            let job = sessions
                .store
                .load_job(id)
                .map_err(storage_error)?
                .ok_or_else(|| invalid("記録がありません"))?;
            let (_, mut state) = decode(&job)?;
            if !state.phase.is_terminal() {
                let recovered =
                    sungeki_jobs::Lifecycle::recover(state.phase, state.progress.clone())
                        .map_err(invalid)?;
                state.phase = recovered.phase();
                state.progress = recovered.progress().cloned();
                state.error = Some(invalid(
                    "hostが中断しました。元音声は変更していません。外部処理・一時ファイルの残存確認は未実施です",
                ));
                save(sessions, &job, &state)?;
            }
        }
        after = ids.last().cloned();
    }
}
#[cfg(test)]
#[path = "loudness_record_tests.rs"]
mod tests;
