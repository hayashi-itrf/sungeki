//! One bounded scheduler tick. Runtime ownership/polling is a separate layer.
use crate::{
    build_record as build,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex};
use sungeki_jobs::{BuildSchedule, BuildStep};
use sungeki_project::JobPhase;
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_storage::StoredJob;

use super::SpeechConnection;
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Advance {
    Progress,
    Waiting,
    Stopped,
}

pub(crate) async fn advance(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    speech: Option<SpeechConnection>,
) -> Result<Advance, ApiError> {
    let (parent, step, fresh, reuse) = {
        let mut sessions = shared.lock().map_err(invalid)?;
        if sessions
            .build_runtime
            .stopping
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(Advance::Stopped);
        }
        let parent = sessions
            .store
            .load_job(id)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("制作記録がありません"))?;
        let (input, state) = build::decode(&parent)?;
        if !matches!(state.phase, JobPhase::Queued | JobPhase::Running) {
            return Ok(Advance::Stopped);
        }
        let step = BuildSchedule::restore(
            input.speech_count,
            state.receipts.len() as u16,
            state.child_id.clone(),
        )
        .map_err(invalid)?
        .step()
        .ok_or_else(|| invalid("未完了の工程がありません"))?;
        let reuse = match step {
            BuildStep::Speech(index) => input
                .continuation
                .as_ref()
                .is_some_and(|source| source.source_for(index).is_some()),
            _ => false,
        };
        let managed = if reuse {
            false
        } else if let BuildStep::Speech(index) = step {
            let (_, current) = sessions.load(&parent.session_id)?;
            super::speech_input::managed::profile_for_clip(
                &input,
                current.history().project(),
                &input.plan.script.speech_ids[usize::from(index)],
            )?
            .is_some()
        } else {
            false
        };
        if matches!(step, BuildStep::Speech(_))
            && state.child_id.is_none()
            && speech.is_none()
            && !managed
            && !reuse
        {
            return Err(ApiError::new(
                ErrorCode::CapabilityUnavailable,
                "合成エンジンの接続確認が必要です",
            ));
        }
        match build::reserve(&mut sessions, &parent)? {
            build::Reservation::New(row) => (row, step, true, reuse),
            build::Reservation::Existing(row) => (row, step, false, reuse),
        }
    };
    match step {
        BuildStep::Prepare => blocking(shared, parent, |s, p| build::prepare(s, p)).await?,
        BuildStep::Compose => {
            build::compose(&mut *shared.lock().map_err(invalid)?, &parent)?;
        }
        BuildStep::Layout => {
            build::layout(&mut *shared.lock().map_err(invalid)?, &parent)?;
        }
        BuildStep::Validate => {
            build::validate_output(shared, &parent).await?;
        }
        BuildStep::Bake => {
            build::finish_bake(&mut *shared.lock().map_err(invalid)?, &parent)?;
        }
        BuildStep::Save => {
            blocking(shared, parent, |s, p| build::save_project(s, p).map(|_| ())).await?
        }
        BuildStep::Speech(_) | BuildStep::Render | BuildStep::Review => {
            if fresh {
                match step {
                    BuildStep::Speech(_) if reuse => {
                        super::reuse_runtime::run(shared, &parent).await?;
                    }
                    BuildStep::Speech(_) => {
                        let context = shared.clone();
                        let expected = parent.clone();
                        super::speech_input::preparation::run(shared, &parent, move |flag| {
                            super::speech_input::managed::start(&context, &expected, speech, &flag)
                                .map(|_| ())
                        })
                        .await?;
                    }
                    BuildStep::Render => {
                        blocking(shared, parent, |s, p| build::start_render(s, p).map(|_| ()))
                            .await?
                    }
                    BuildStep::Review => {
                        blocking(shared, parent, |s, p| build::start_review(s, p).map(|_| ()))
                            .await?
                    }
                    _ => unreachable!(),
                }
                return Ok(Advance::Waiting);
            }
            let child = child_ready(shared, &parent, step)?;
            let Some(child) = child else {
                return Ok(Advance::Waiting);
            };
            match step {
                BuildStep::Speech(_) => {
                    build::apply_speech(shared, &parent.id, &child.id).await?;
                }
                BuildStep::Render => {
                    blocking(shared, parent, |s, p| {
                        build::accept_render(s, p).map(|_| ())
                    })
                    .await?
                }
                BuildStep::Review => {
                    blocking(shared, parent, |s, p| {
                        build::accept_review(s, p).map(|_| ())
                    })
                    .await?
                }
                _ => unreachable!(),
            }
        }
    }
    Ok(Advance::Progress)
}
async fn blocking(
    shared: &Arc<Mutex<Sessions>>,
    parent: StoredJob,
    f: impl FnOnce(&Arc<Mutex<Sessions>>, &StoredJob) -> Result<(), ApiError> + Send + 'static,
) -> Result<(), ApiError> {
    let shared = shared.clone();
    tokio::task::spawn_blocking(move || f(&shared, &parent))
        .await
        .map_err(invalid)?
}
fn child_ready(
    shared: &Arc<Mutex<Sessions>>,
    parent: &StoredJob,
    step: BuildStep,
) -> Result<Option<StoredJob>, ApiError> {
    let sessions = shared.lock().map_err(invalid)?;
    let (_, state) = build::decode(parent)?;
    let id = state
        .child_id
        .as_ref()
        .ok_or_else(|| invalid("子処理が未予約です"))?;
    let child = sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| {
            invalid("予約済み子処理がありません。実行状態を確認してください。自動再起動はしません")
        })?;
    let (owner, phase, error) = match step {
        BuildStep::Speech(_) => {
            let (i, s) = crate::speech_journal::decode_input(&child)?;
            (i.parent_job_id().map(str::to_owned), s.phase, s.error)
        }
        BuildStep::Render => {
            let (i, s) = crate::render_record::decode(&child)?;
            (i.parent_job_id, s.phase, s.error)
        }
        BuildStep::Review => {
            let (i, s) = crate::review_record::decode(&child)?;
            (i.parent_job_id, s.phase, s.error)
        }
        _ => return Err(invalid("子処理の工程ではありません")),
    };
    if owner.as_deref() != Some(parent.id.as_str()) || child.session_id != parent.session_id {
        return Err(invalid("子処理の所有が一致しません"));
    }
    match phase {
        JobPhase::Succeeded => Ok(Some(child)),
        JobPhase::Queued | JobPhase::Running => Ok(None),
        _ => Err(child_error(step, &child.id, phase, error)),
    }
}

fn child_error(step: BuildStep, id: &str, phase: JobPhase, error: Option<ApiError>) -> ApiError {
    let stage = match step {
        BuildStep::Speech(_) => "音声生成",
        BuildStep::Render => "動画出力",
        BuildStep::Review => "確認レポート",
        _ => unreachable!(),
    };
    let mut error = error.unwrap_or_else(|| match phase {
        JobPhase::Cancelling => ApiError::new(
            ErrorCode::IoError,
            "取消処理の停止確認がまだ完了していません",
        ),
        JobPhase::Cancelled => ApiError::new(ErrorCode::Cancelled, "処理が取り消されました"),
        JobPhase::Interrupted => ApiError::new(ErrorCode::IoError, "処理が中断しました"),
        _ => ApiError::new(
            ErrorCode::IoError,
            "処理に失敗しました。詳細は記録されていません",
        ),
    });
    error.message = format!("{stage}で制作が停止しました：{}", error.message);
    error.details.insert("build_child_job_id".into(), id.into());
    error
        .details
        .insert("build_child_stage".into(), stage.into());
    error
        .details
        .insert("build_child_phase".into(), format!("{phase:?}"));
    error
        .details
        .insert("build_child_retryable".into(), error.retryable.to_string());
    // A retryable child does not make this interrupted parent resumable.
    error.retryable = false;
    error.suggested_actions.insert(
        0,
        "保存済み作品と子処理の記録を確認し、原因を修正してください。一括制作は自動再開しません"
            .into(),
    );
    error
}
