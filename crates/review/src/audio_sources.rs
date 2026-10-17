//! Bounded original-byte verification; compressed decoding and loudness remain separate.
use crate::{ImageReviewError, Issue, Severity};
use std::path::Path;
use sungeki_media::{WaveformCacheError, inspect_wav, read_audio_original_with_cancel};
use sungeki_project::{AssetLocator, AssetProbe, Project};

pub fn review_audio_sources(
    project: &Project,
    max_bytes: u64,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<Issue>, ImageReviewError> {
    let mut remaining = max_bytes;
    let mut issues = Vec::new();
    for (index, asset) in project.assets.iter().enumerate() {
        if cancelled() {
            return Err(ImageReviewError::Cancelled);
        }
        let AssetProbe::Audio(probe) = &asset.probe else {
            continue;
        };
        let outcome = if probe.byte_length > remaining
            || probe.byte_length > sungeki_media::MAX_WAV_BYTES as u64
        {
            Some((
                Severity::Warning,
                "AUDIO_SOURCE_NOT_INSPECTED",
                "音声原本が検査容量の上限を超えるため未検査です。".into(),
            ))
        } else if let AssetLocator::Absolute { path } = &asset.locator {
            remaining -= probe.byte_length;
            let captured = if probe.encoding.is_some() {
                sungeki_media::source_identity(Path::new(path), probe.byte_length, &cancelled)
                    .map_err(|error| match error {
                        WaveformCacheError::Wav(sungeki_media::WavError::TooLarge) => {
                            WaveformCacheError::SourceChanged
                        }
                        other => other,
                    })
                    .and_then(|(hash, size)| {
                        if hash == probe.sha256 && size == probe.byte_length {
                            Ok(Vec::new())
                        } else {
                            Err(WaveformCacheError::SourceChanged)
                        }
                    })
            } else {
                read_audio_original_with_cancel(Path::new(path), probe, &cancelled)
            };
            match captured {
                Ok(bytes) => {
                    if asset.content_hash.as_deref() != Some(probe.sha256.as_str()) {
                        Some((
                            Severity::Error,
                            "AUDIO_METADATA_MISMATCH",
                            "音声の登録hashが一致しません。".into(),
                        ))
                    } else if probe.encoding.is_none() {
                        match inspect_wav(bytes) {
                            Ok(wav)
                                if wav.info().sample_rate == probe.sample_rate
                                    && wav.info().channels == probe.channels
                                    && wav.info().frames == probe.frames
                                    && wav.info().duration_us == probe.duration_us.0 =>
                            {
                                None
                            }
                            _ => Some((
                                Severity::Error,
                                "AUDIO_METADATA_MISMATCH",
                                "音声原本の形式・尺が登録情報と一致しません。再登録してください。"
                                    .into(),
                            )),
                        }
                    } else {
                        None
                    } // Encoded hash is verified; no decoded-metadata claim.
                }
                Err(WaveformCacheError::Cancelled) => return Err(ImageReviewError::Cancelled),
                Err(WaveformCacheError::SourceChanged) => Some((
                    Severity::Error,
                    "AUDIO_SOURCE_CHANGED",
                    "音声原本が登録時から、または読込み中に変更されています。再登録してください。"
                        .into(),
                )),
                Err(WaveformCacheError::Io(error))
                    if error.kind() == std::io::ErrorKind::NotFound =>
                {
                    Some((
                        Severity::Error,
                        "AUDIO_SOURCE_MISSING",
                        "音声原本が見つかりません。".into(),
                    ))
                }
                Err(error) => Some((
                    Severity::Error,
                    "AUDIO_SOURCE_UNREADABLE",
                    format!("音声原本を検査できません: {error}"),
                )),
            }
        } else {
            Some((
                Severity::Warning,
                "AUDIO_SOURCE_NOT_INSPECTED",
                "音声原本の絶対パスが未解決のため未検査です。".into(),
            ))
        };
        if let Some((severity, code, message)) = outcome {
            issues.push(Issue {
                severity,
                code: code.into(),
                field: format!("/assets/{index}"),
                asset_id: Some(asset.id.clone()),
                clip_id: None,
                scene_id: None,
                start_us: None,
                end_us: None,
                message,
            });
        }
    }
    if cancelled() {
        return Err(ImageReviewError::Cancelled);
    }
    Ok(issues)
}
