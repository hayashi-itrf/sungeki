//! Freeze the exact pre-master mix once for measurement and final encoding.
use crate::{
    RenderError, RenderPlan,
    execute::{Runner, strings},
    originals::{cancelled, write_new},
    plan::error,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, time::Instant};

pub(crate) fn freeze(
    plan: &RenderPlan,
    runner: &Runner<'_>,
    resources: &BTreeMap<String, Value>,
) -> Result<String, RenderError> {
    let mut args = Vec::new();
    for cue in &plan.scene.audio {
        let path = resources
            .get(&cue.source.resource_id().expect("validated audio"))
            .and_then(|v| {
                if v["probe"]["info"]["encoding"].is_object() {
                    v["decoded_audio_path"].as_str()
                } else {
                    v["path"].as_str()
                }
            })
            .ok_or_else(|| error("音声原本パスが不正です"))?;
        args.extend(strings(&["-i", path]));
    }
    write_new(
        &runner.directory.join("audio.filter"),
        plan.audio_graph_with_offset(0).as_bytes(),
    )?;
    args.extend(strings(&[
        "-filter_complex_script",
        "audio.filter",
        "-map",
        "[audio]",
        "-c:a",
        "pcm_f32le",
        "-ar",
        "48000",
        "-ac",
        "2",
        "-map_metadata",
        "-1",
        "mix.wav",
    ]));
    runner.ffmpeg(args)?;
    let (hash, byte_length, probe) = inspect(runner, "mix.wav", plan.audio_samples())?;
    write_new(
        &runner.directory.join("mix.json"),
        &serde_json::to_vec(&json!({
            "version": 1, "stage": "pre_master", "path": "mix.wav", "sample_rate": 48000,
            "channels": 2, "frames": plan.audio_samples(), "byte_length": byte_length,
            "sha256": hash, "probe": probe,
        }))?,
    )?;
    Ok(hash)
}

pub(crate) fn inspect(
    runner: &Runner<'_>,
    name: &str,
    frames: u64,
) -> Result<(String, u64, Value), RenderError> {
    let data = runner.run(
        runner.runtime.ffprobe(),
        strings(&["-v", "error", "-show_streams", "-of", "json", name]),
    )?;
    let probe: Value = serde_json::from_slice(&data)?;
    let streams = probe["streams"]
        .as_array()
        .ok_or_else(|| error("ミックス音声の検査結果が不正です"))?;
    let valid = streams.len() == 1 && {
        let a = &streams[0];
        a["codec_type"] == "audio"
            && a["codec_name"] == "pcm_f32le"
            && a["sample_rate"] == "48000"
            && a["channels"] == 2
            && a["time_base"] == "1/48000"
            && a["duration_ts"].as_u64() == Some(frames)
    };
    if !valid {
        return Err(error(
            "ミックス音声の形式・サンプル数が出力計画と一致しません",
        ));
    }
    let path = runner.directory.join(name);
    let mut file = fs::File::open(&path)?;
    file.sync_all()?;
    let byte_length = file.metadata()?.len();
    // At most two hours of stereo f32 PCM plus a bounded WAV header.
    let pcm_bytes = frames * 8;
    if byte_length < pcm_bytes || byte_length > pcm_bytes + 65536 {
        return Err(error("ミックス音声のファイルサイズが不正です"));
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        cancelled(runner.flag)?;
        if Instant::now() >= runner.end {
            return Err(error("音声固定の実行期限を超えました"));
        }
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok((format!("{:x}", digest.finalize()), byte_length, probe))
}
