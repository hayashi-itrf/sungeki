use crate::sessions::Sessions;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use sungeki_media::{WaveformCache, WaveformCacheError};
use sungeki_project::{AssetLocator, AssetProbe};
use sungeki_protocol::*;
static CACHE: Mutex<Option<WaveformCache>> = Mutex::new(None);
pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: AnalyzeAudioArgs,
) -> Result<SessionResponse, ApiError> {
    if !(1..=65536).contains(&args.base_frames)
        || !(1..=4096).contains(&args.count)
        || args.channel > 1
        || args.level > 18
    {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "波形の解像度・channel・取得区間数が範囲外です",
        ));
    }
    let (asset, cancel, database) = {
        let sessions = shared.lock().map_err(ApiError::io)?;
        let (_, current) = sessions.load(&args.session_id)?;
        if current.history().revision() != args.expected_revision {
            return Err(conflict());
        }
        let asset = current
            .history()
            .project()
            .assets
            .iter()
            .find(|a| a.id == args.asset_id)
            .cloned()
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "音声素材がありません"))?;
        (
            asset,
            sessions.render_runtime.stopping.clone(),
            sessions.review_database_path.clone(),
        )
    };
    let AssetProbe::Audio(probe) = asset.probe else {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "音声素材を指定してください",
        ));
    };
    let AssetLocator::Absolute { path } = asset.locator else {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "波形解析は絶対パスの元音声に対応しています",
        ));
    };
    let config = if probe.encoding.is_some() {
        Some(crate::audio_decode::Configuration::load(&database)?)
    } else {
        None
    };
    let runtime = config.as_ref().map(|c| c.runtime());
    let mut guard = CACHE.try_lock().map_err(|_| {
        let mut e = ApiError::new(
            ErrorCode::Conflict,
            "別の音声を解析中です。再試行してください",
        );
        e.retryable = true;
        e
    })?;
    let cache = guard.get_or_insert_with(|| {
        WaveformCache::new(32 * 1024 * 1024, 16).expect("fixed valid cache limits")
    });
    let result = cache
        .get(
            Path::new(&path),
            &probe,
            args.base_frames,
            runtime.as_ref(),
            &cancel,
        )
        .map_err(error)?;
    let wave = &result.waveform;
    let window = wave
        .window(
            args.level as usize,
            usize::from(args.channel),
            args.start_bucket as usize,
            args.count as usize,
        )
        .map_err(|e| ApiError::new(ErrorCode::ValidationFailed, e.to_string()))?;
    let analysis = AudioAnalysis {
        session_id: args.session_id.clone(),
        source_revision: args.expected_revision,
        asset_id: args.asset_id,
        source_sha256: wave.source_sha256.clone(),
        version: wave.version,
        signal: AudioAnalysisSignal::SourcePreGain,
        sample_rate: wave.sample_rate,
        channels: wave.channels,
        source_frames: wave.source_frames,
        rms: wave.rms.clone(),
        sample_peak: wave.sample_peak.clone(),
        lufs_measured: false,
        true_peak_measured: false,
        cache_hit: result.cache_hit,
        levels: wave
            .levels()
            .iter()
            .map(|l| AudioWaveformLevel {
                frames_per_bucket: l.frames_per_bucket,
                bucket_count: l.bucket_count() as u32,
            })
            .collect(),
        level: args.level,
        channel: args.channel,
        start_bucket: args.start_bucket,
        start_frame: window.start_frame,
        end_frame: window.end_frame,
        peaks: window.peaks.iter().map(|p| [p.min, p.max]).collect(),
    };
    drop(guard);
    // Recheck the snapshot after file I/O. A cached result is not permission to return stale edits.
    let sessions = shared.lock().map_err(ApiError::io)?;
    let (_, current) = sessions.load(&args.session_id)?;
    if current.history().revision() != args.expected_revision {
        return Err(conflict());
    }
    Ok(SessionResponse::AudioAnalyzed { analysis })
}
fn conflict() -> ApiError {
    ApiError::new(ErrorCode::Conflict, "解析対象の作品が更新されています")
}
pub(crate) fn error(cause: WaveformCacheError) -> ApiError {
    let code = match &cause {
        WaveformCacheError::SourceChanged => ErrorCode::ExternalModification,
        WaveformCacheError::Cancelled => ErrorCode::Cancelled,
        WaveformCacheError::MissingRuntime => ErrorCode::CapabilityUnavailable,
        WaveformCacheError::Decode(
            sungeki_media::Mp3DecodeError::Cancelled
            | sungeki_media::Mp3DecodeError::Stopped(sungeki_media::StopReason::Cancelled),
        ) => ErrorCode::Cancelled,
        WaveformCacheError::Io(e) if e.kind() == std::io::ErrorKind::NotFound => {
            ErrorCode::AssetMissing
        }
        WaveformCacheError::Io(_) => ErrorCode::IoError,
        _ => ErrorCode::ValidationFailed,
    };
    ApiError::new(code, cause.to_string())
}
