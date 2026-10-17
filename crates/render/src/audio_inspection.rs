//! Read-only measurement of the exact PCM mix that would feed the movie encoder.
use crate::{RenderError, RenderPlan, RenderRuntime, originals, plan::error};
use std::{path::Path, sync::atomic::AtomicBool};
use sungeki_media::LoudnessAnalysis;

pub fn inspect_output_audio(
    plan: &RenderPlan,
    runtime: &RenderRuntime,
    directory: &Path,
    flag: &AtomicBool,
) -> Result<LoudnessAnalysis, RenderError> {
    let runner = crate::prepare::runner(runtime, directory, flag)?;
    let pcm = (plan.audio_samples() + 1024) * 8 + 65536;
    sungeki_media::ensure_available_space(directory, pcm * 4 + 1024 * 1024 * 1024)?;
    // The host supplies the same speech-freshness-checked audio-only plan as preview.
    let resources = originals::capture(plan, &runner)?;
    let prepared = crate::prepared_visuals::PreparedVisuals::new(plan, &runner, resources)?;
    let hash = crate::audio_mix::freeze(plan, &runner, &prepared.resources)?;
    let normalized = crate::master_audio::prepare(plan, &runner, &hash)?;
    let measured = if let Some(report) = normalized {
        serde_json::from_value(report["master"]["measurement"].clone()).map_err(error)?
    } else {
        crate::master_audio::measure(plan, &runner, "mix.wav", &hash, false)?
    };
    originals::cancelled(flag)?;
    if std::time::Instant::now() >= runner.end {
        return Err(error("音量検査の実行期限を超えました"));
    }
    Ok(measured)
}
