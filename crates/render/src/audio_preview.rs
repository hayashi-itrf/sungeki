//! Audio-only output, sharing the video's frozen mix, master and AAC verification.
use crate::{
    AudioRuntime, RenderError, RenderPlan,
    execute::{Runner, strings},
    originals,
    plan::error,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};

pub const AUDIO_PREVIEW_CHUNK: usize = 256 * 1024;
#[derive(Serialize)]
pub struct AudioPreviewArtifact {
    pub path: PathBuf,
    pub sha256: String,
    pub byte_length: u64,
    pub chunk_sha256: Vec<String>,
    pub master: Option<serde_json::Value>,
}
pub fn audio_preview_plan(mut plan: RenderPlan) -> RenderPlan {
    let ids: std::collections::BTreeSet<_> = plan
        .scene
        .audio
        .iter()
        .filter_map(|cue| cue.source.asset_id().map(str::to_owned))
        .collect();
    plan.sources.retain(|id, _| ids.contains(id));
    // Visual-only assets were removed above; do not bake their VRM layers.
    // Video sources used for audio remain available for stream decoding.
    plan.scene.layers.clear();
    plan
}
pub fn audio_preview_staged(
    plan: &RenderPlan,
    runtime: &AudioRuntime,
    directory: &Path,
    flag: &AtomicBool,
) -> Result<AudioPreviewArtifact, RenderError> {
    if !directory.is_absolute()
        || !fs::symlink_metadata(directory)?.is_dir()
        || fs::read_dir(directory)?.next().is_some()
        || runtime.deadline.is_zero()
        || runtime.deadline.as_secs() > 86400
    {
        return Err(error("確認音声の専用作業領域または期限が不正です"));
    }
    let runner = Runner {
        runtime: runtime.into(),
        directory,
        flag,
        end: Instant::now() + runtime.deadline,
    };
    originals::cancelled(flag)?;
    let pcm = (plan.audio_samples() + 1024) * 8 + 65536;
    sungeki_media::ensure_available_space(directory, pcm * 4 + 1024 * 1024 * 1024)?;
    let resources = originals::capture(plan, &runner)?;
    let prepared = crate::prepared_visuals::PreparedVisuals::new(plan, &runner, resources)?;
    let hash = crate::audio_mix::freeze(plan, &runner, &prepared.resources)?;
    let master = crate::master_audio::prepare(plan, &runner, &hash)?;
    let name = if master.is_some() {
        "master.wav"
    } else {
        "mix.wav"
    };
    runner.ffmpeg(strings(&[
        "-i",
        name,
        "-map",
        "0:a:0",
        "-c:a",
        "aac",
        "-b:a",
        "192k",
        "-movflags",
        "+faststart",
        "-map_metadata",
        "-1",
        "output.mp4",
    ]))?;
    let probe = runner.run(
        &runtime.ffprobe,
        strings(&["-v", "error", "-show_streams", "-of", "json", "output.mp4"]),
    )?;
    let probe: serde_json::Value = serde_json::from_slice(&probe)?;
    let streams = probe["streams"]
        .as_array()
        .ok_or_else(|| error("音声情報がありません"))?;
    if streams.len() != 1
        || streams[0]["codec_name"] != "aac"
        || streams[0]["sample_rate"] != "48000"
        || streams[0]["channels"] != 2
    {
        return Err(error("確認音声の形式が不正です"));
    }
    let frames = streams[0]["duration_ts"]
        .as_u64()
        .ok_or_else(|| error("音声の尺が不正です"))?;
    if streams[0]["time_base"] != "1/48000" || frames.abs_diff(plan.audio_samples()) > 1024 {
        return Err(error("確認音声の尺が計画と一致しません"));
    }
    let master = master
        .map(|report| crate::master_audio::verify_encoded(plan, &runner, report))
        .transpose()?;
    let path = directory.join("output.mp4");
    let mut file = fs::File::open(&path)?;
    let byte_length = file.metadata()?.len();
    if byte_length == 0 || byte_length > 512 * 1024 * 1024 {
        return Err(error("確認音声のサイズが不正です"));
    }
    let mut digest = Sha256::new();
    let mut chunks = Vec::new();
    let mut remaining = byte_length;
    let mut buffer = vec![0; AUDIO_PREVIEW_CHUNK];
    while remaining > 0 {
        originals::cancelled(flag)?;
        if Instant::now() >= runner.end {
            return Err(error("確認音声の実行期限を超えました"));
        }
        let count = remaining.min(AUDIO_PREVIEW_CHUNK as u64) as usize;
        file.read_exact(&mut buffer[..count])?;
        digest.update(&buffer[..count]);
        chunks.push(format!("{:x}", Sha256::digest(&buffer[..count])));
        remaining -= count as u64;
    }
    file.sync_all()?;
    Ok(AudioPreviewArtifact {
        path,
        byte_length,
        sha256: format!("{:x}", digest.finalize()),
        chunk_sha256: chunks,
        master,
    })
}
