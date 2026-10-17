use crate::{
    RenderError, RenderPlan,
    execute::{Runner, strings},
    plan::error,
};
use serde_json::Value;

pub(crate) fn inspect(plan: &RenderPlan, runner: &Runner<'_>) -> Result<Value, RenderError> {
    let data = runner.run(
        runner.runtime.ffprobe(),
        strings(&[
            "-v",
            "error",
            "-count_frames",
            "-show_streams",
            "-show_chapters",
            "-of",
            "json",
            "output.mp4",
        ]),
    )?;
    let probe: Value = serde_json::from_slice(&data)?;
    let streams = probe["streams"]
        .as_array()
        .ok_or_else(|| error("映像検証結果が不正です"))?;
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video")
        .ok_or_else(|| error("映像がありません"))?;
    let audio = streams
        .iter()
        .find(|s| s["codec_type"] == "audio")
        .ok_or_else(|| error("音声がありません"))?;
    let expected_seconds = plan.audio_samples() as f64 / 48000.0;
    let duration = |stream: &Value| {
        stream["duration"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(f64::NAN)
    };
    // FFmpeg adds one QuickTime text data track for MP4 chapters. It is not a
    // subtitle/video stream, and must not weaken the two media stream checks.
    let chapter_tracks = streams
        .iter()
        .filter(|s| {
            s["codec_type"] == "data"
                && s["codec_name"] == "bin_data"
                && s["codec_tag_string"] == "text"
        })
        .count();
    if chapter_tracks != usize::from(!plan.chapters.is_empty())
        || streams.len() != 2 + chapter_tracks
        || streams
            .iter()
            .filter(|s| s["codec_type"] == "video")
            .count()
            != 1
        || streams
            .iter()
            .filter(|s| s["codec_type"] == "audio")
            .count()
            != 1
        || video["codec_name"] != "h264"
        || video["pix_fmt"] != "yuv420p"
        || video["width"] != plan.scene.canvas.width
        || video["height"] != plan.scene.canvas.height
        || video["nb_read_frames"].as_str() != Some(plan.frame_count().to_string().as_str())
        || audio["codec_name"] != "aac"
        || audio["sample_rate"] != "48000"
        || audio["channels"] != 2
        || !duration(video).is_finite()
        || !duration(audio).is_finite()
        || (duration(video) - expected_seconds).abs() > 0.005
        || (duration(audio) - expected_seconds).abs() > 0.025
    {
        return Err(error(
            "完成ファイルの映像・音声・尺が出力計画と一致しません",
        ));
    }
    crate::chapters::verify(&plan.chapters, &probe)?;
    runner.ffmpeg(strings(&[
        "-i",
        "output.mp4",
        "-map",
        "0:v:0",
        "-map",
        "0:a:0",
        "-f",
        "null",
        "-",
    ]))?;
    Ok(probe)
}
