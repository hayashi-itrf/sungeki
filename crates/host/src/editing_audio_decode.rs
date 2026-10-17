use std::{path::Path, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::video_probe::{ProbeRuntime, frames::PreparedVideo};
use sungeki_project::{AssetProbe, VideoProbe};
use sungeki_protocol::{ApiError, ErrorCode};
const LIMIT: u64 = 128 * 1024 * 1024;
fn invalid(value: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, value.to_string())
}
pub(super) fn decode(
    path: &Path,
    probe: &AssetProbe,
    stream: Option<u32>,
    database: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, ApiError> {
    match probe {
        AssetProbe::Audio(probe) => {
            if probe.frames * u64::from(probe.channels) * 4 > LIMIT {
                return Err(invalid("音声が編集再生の128 MiB上限を超えています"));
            }
            let config = if probe.encoding.is_some() {
                Some(crate::audio_decode::Configuration::load(database)?)
            } else {
                None
            };
            let runtime = config.as_ref().map(|c| c.runtime());
            let wav = sungeki_media::decode_audio_original(path, probe, runtime.as_ref(), cancel)
                .map_err(crate::audio_decode::error)?;
            Ok(wav.bytes().to_vec())
        }
        AssetProbe::Video(probe) => video(
            path,
            probe,
            stream.ok_or_else(|| invalid("音声ストリームが未選択です"))?,
            database,
            cancel,
        ),
        _ => Err(invalid("音声または動画を指定してください")),
    }
}
fn video(
    path: &Path,
    probe: &VideoProbe,
    stream: u32,
    database: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, ApiError> {
    let duration = probe
        .metadata
        .video
        .clock
        .duration_us()
        .map_err(invalid)?
        .ok_or_else(|| invalid("動画尺が不明です"))?;
    if duration.0.saturating_mul(48000 * 8) / 1_000_000 + 44 > LIMIT {
        return Err(invalid("動画音声が編集再生の128 MiB上限を超えています"));
    }
    let config = crate::audio_decode::Configuration::load(database)?;
    let runtime = config.runtime();
    let saved = crate::runtime_settings::load(database)?;
    let ffprobe = crate::runtime_settings::file(
        std::env::var_os("SUNGEKI_RENDER_FFPROBE").map(Into::into),
        saved.ffprobe.as_deref(),
        "FFprobe",
    )?;
    let source = PreparedVideo::open(
        path,
        probe,
        &ProbeRuntime {
            ffprobe: &ffprobe,
            root: runtime.root,
            timeout: Duration::from_secs(60),
            max_source_bytes: LIMIT,
        },
        cancel,
    )
    .map_err(invalid)?;
    let audio = source
        .decode_audio(stream, runtime.ffmpeg, Duration::from_secs(60), cancel)
        .map_err(invalid)?;
    // The existing media layer removes both the decoded audio and captured video on drop.
    std::fs::read(audio.path()).map_err(ApiError::io)
}
