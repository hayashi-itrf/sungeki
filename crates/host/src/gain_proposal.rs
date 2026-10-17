use crate::{
    loudness_record::{decode, invalid},
    sessions::Sessions,
};
use std::sync::{Arc, Mutex, atomic::Ordering};
use sungeki_project::{AssetLocator, AssetProbe, ClipContent, JobPhase};
use sungeki_protocol::*;
pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: ProposeSpeechGainArgs,
) -> Result<SessionResponse, ApiError> {
    let (input, output, current_gain_db, cancel) = {
        let s = shared.lock().map_err(invalid)?;
        let row = crate::loudness_api::record(&s, &args.job_id)?;
        let (input, state) = decode(&row)?;
        if row.session_id != args.session_id || input.source_revision != args.expected_revision {
            return Err(conflict(
                "測定時点と提案対象が一致しません。再測定してください",
            ));
        }
        if state.phase != JobPhase::Succeeded {
            return Err(conflict("完了済みの測定が必要です"));
        }
        let output = state
            .output
            .ok_or_else(|| invalid("測定結果がありません"))?;
        let (_, current) = s.load(&args.session_id)?;
        if current.history().revision() != args.expected_revision {
            return Err(conflict("作品が更新されています"));
        }
        let project = current.history().project();
        let clip = project
            .clips
            .iter()
            .find(|c| c.id == args.clip_id)
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "発話が見つかりません"))?;
        let ClipContent::Speech {
            audio: Some(audio),
            gain_db,
            ..
        } = &clip.content
        else {
            return Err(conflict("合成済みの発話を指定してください"));
        };
        if audio.asset_id != input.asset_id
            || !sungeki_speech::audio_reference_matches(project, &clip.id).unwrap_or(false)
        {
            return Err(conflict("測定音声と発話が一致しません"));
        }
        let asset = project
            .assets
            .iter()
            .find(|a| a.id == input.asset_id)
            .ok_or_else(|| conflict("元音声がありません"))?;
        if asset.probe != AssetProbe::Audio(input.probe.clone())
            || asset.locator
                != (AssetLocator::Absolute {
                    path: input.source_path.to_string_lossy().into_owned(),
                })
        {
            return Err(conflict("元音声の登録が変更されています"));
        }
        (input, output, *gain_db, s.loudness_runtime.stopping.clone())
    };
    let estimate =
        sungeki_media::propose_speech_gain(&output, args.target_lufs, args.max_true_peak_db)
            .map_err(|e| ApiError::new(ErrorCode::ValidationFailed, e.to_string()))?;
    // No session lock across bounded file I/O. Recheck revision after verifying bytes.
    let _bytes = sungeki_media::read_audio_original(&input.source_path, &input.probe, &cancel)
        .map_err(crate::audio_analysis::error)?;
    let s = shared.lock().map_err(invalid)?;
    let (_, current) = s.load(&args.session_id)?;
    if current.history().revision() != args.expected_revision {
        return Err(conflict("検証中に作品が更新されています"));
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(ApiError::new(ErrorCode::Cancelled, "hostは終了処理中です"));
    }
    let gain_db = estimate.gain_db;
    let estimate = serde_json::from_value(serde_json::to_value(estimate).map_err(invalid)?)
        .map_err(invalid)?;
    Ok(SessionResponse::SpeechGainProposed {
        proposal: SpeechGainProposal {
            session_id: args.session_id,
            source_revision: args.expected_revision,
            job_id: args.job_id,
            clip_id: args.clip_id.clone(),
            asset_id: input.asset_id,
            source_sha256: input.probe.sha256,
            current_gain_db,
            estimate,
            command: sungeki_editor::Command::SetSpeechGain {
                clip_id: args.clip_id,
                gain_db: Some(gain_db),
            },
        },
    })
}
fn conflict(message: &str) -> ApiError {
    ApiError::new(ErrorCode::Conflict, message)
}
