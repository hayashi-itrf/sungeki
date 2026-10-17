//! Measure, adjust, then verify the exact PCM supplied to the output encoder.
use crate::{
    RenderError, RenderPlan, audio_mix,
    execute::{Runner, strings},
    master_policy,
    originals::write_new,
    plan::error,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use sungeki_media::{
    LoudnessAnalysis, MixFrameLimit, ensure_available_space, measure_render_mix_loudness,
};

pub(crate) fn measure(
    plan: &RenderPlan,
    runner: &Runner<'_>,
    name: &str,
    hash: &str,
    decoded: bool,
) -> Result<LoudnessAnalysis, RenderError> {
    let timeout = runner
        .end
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| error("音量測定の期限を超えました"))?
        .min(Duration::from_secs(3600));
    measure_render_mix_loudness(
        &runner.directory.join(name),
        hash,
        runner.runtime.ffmpeg(),
        runner.directory,
        timeout,
        runner.flag,
        MixFrameLimit::for_video(plan.scene.duration_us, &plan.scene.canvas.fps, decoded)
            .map_err(error)?,
    )
    .map_err(error)
}
pub(crate) fn prepare(
    plan: &RenderPlan,
    runner: &Runner<'_>,
    hash: &str,
) -> Result<Option<Value>, RenderError> {
    let Some(target) = &plan.scene.audio_mix.normalization else {
        return Ok(None);
    };
    ensure_available_space(
        runner.directory,
        (plan.audio_samples() * 8 + 65536) * 2 + 64 * 1024 * 1024,
    )?;
    let input = measure(plan, runner, "mix.wav", hash, false)?;
    let gain = master_policy::gain(&input, target)?;
    runner.ffmpeg(strings(&[
        "-i",
        "mix.wav",
        "-map",
        "0:a:0",
        "-af",
        &format!("volume={gain:.12}dB:precision=double"),
        "-c:a",
        "pcm_f32le",
        "-ar",
        "48000",
        "-ac",
        "2",
        "-map_metadata",
        "-1",
        "master.wav",
    ]))?;
    let (master_hash, bytes, _) = audio_mix::inspect(runner, "master.wav", plan.audio_samples())?;
    let master = measure(plan, runner, "master.wav", &master_hash, false)?;
    master_policy::verify(&master, target)?;
    let report = json!({ "version": 1, "method": "constant_gain_verified",
        "target": target, "gain_db": gain, "lufs_tolerance": master_policy::LUFS_TOLERANCE,
        "peak_margin_db": master_policy::PEAK_MARGIN, "input": input,
        "master": {"path":"master.wav", "byte_length":bytes, "measurement":master},
        "encoded_verified":false });
    write_new(
        &runner.directory.join("master.json"),
        &serde_json::to_vec(&report)?,
    )?;
    Ok(Some(report))
}

pub(crate) fn verify_encoded(
    plan: &RenderPlan,
    runner: &Runner<'_>,
    mut report: Value,
) -> Result<Value, RenderError> {
    let temporary = crate::decoded_audio::Temporary::new(runner.directory)?;
    let decoded_runner = Runner {
        runtime: runner.runtime,
        directory: temporary.path(),
        flag: runner.flag,
        end: runner.end,
    };
    let original_runner = runner;
    let runner = &decoded_runner;
    ensure_available_space(
        runner.directory,
        ((plan.audio_samples() + 1024) * 8 + 65536) * 2 + 64 * 1024 * 1024,
    )?;
    // Measure the full decoded AAC, including codec padding, rather than trimming
    // the measurement to conceal an encoder overshoot or unexpected trailing audio.
    runner.ffmpeg(strings(&[
        "-i",
        "../output.mp4",
        "-map",
        "0:a:0",
        "-vn",
        "-c:a",
        "pcm_f32le",
        "-ar",
        "48000",
        "-ac",
        "2",
        "-map_metadata",
        "-1",
        "encoded.wav",
    ]))?;
    let probe = runner.run(
        runner.runtime.ffprobe(),
        strings(&["-v", "error", "-show_streams", "-of", "json", "encoded.wav"]),
    )?;
    let probe: Value = serde_json::from_slice(&probe)?;
    let frames = probe["streams"][0]["duration_ts"]
        .as_u64()
        .ok_or_else(|| error("復号音声の尺が不正です"))?;
    if frames.abs_diff(plan.audio_samples()) > 1024 {
        return Err(error("復号音声の尺が計画と一致しません"));
    }
    let (hash, _, _) = audio_mix::inspect(runner, "encoded.wav", frames)?;
    let encoded = measure(plan, runner, "encoded.wav", &hash, true)?;
    master_policy::verify(
        &encoded,
        plan.scene
            .audio_mix
            .normalization
            .as_ref()
            .expect("enabled"),
    )?;
    report["encoded"] = serde_json::to_value(encoded)?;
    report["encoded_verified"] = json!(true);
    temporary.finish()?;
    write_new(
        &original_runner.directory.join("master-output.json"),
        &serde_json::to_vec(&report)?,
    )?;
    Ok(report)
}
