//! Local HTTP synthesis uses the same durable jobs, leases and editor application.
use crate::{
    sessions::{Sessions, storage_error},
    speech_journal::{Input, decode_input},
    speech_state::invalid,
    speech_worker::Worker,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse, StartSpeechArgs};
use sungeki_speech::{
    SpeechResponse,
    local_http::{LocalSpeechClient, LocalSpeechJobInput},
    voicevox::TransportError,
};
pub(crate) fn start(
    shared: &Arc<Mutex<Sessions>>,
    args: StartSpeechArgs,
) -> Result<SessionResponse, ApiError> {
    let endpoint = sungeki_speech::voicevox::canonical_endpoint(&args.base_url).map_err(invalid)?;
    let input = {
        let sessions = shared.lock().map_err(invalid)?;
        if let Some(record) = sessions
            .store
            .load_job(&args.job_id)
            .map_err(storage_error)?
        {
            if record.kind != sungeki_storage::JobKind::Speech {
                return Err(conflict());
            }
            let (Input::Local(input), _) = decode_input(&record)? else {
                return Err(conflict());
            };
            if record.session_id != args.session_id
                || input.source_revision() != args.expected_revision
                || input.clip_id() != args.clip_id
                || input.endpoint() != endpoint
                || input.engine_version() != args.engine_version
            {
                return Err(conflict());
            }
            input
        } else {
            let (_, current) = sessions.load(&args.session_id)?;
            if current.history().revision() != args.expected_revision {
                return Err(conflict());
            }
            LocalSpeechJobInput::new(
                args.expected_revision,
                current.history().project(),
                &args.clip_id,
                &endpoint,
                &args.engine_version,
            )
            .map_err(invalid)?
        }
    };
    crate::speech_runtime::start_prepared(
        shared,
        args.session_id,
        args.job_id.clone(),
        &Input::Local(input),
        Worker::Local,
        None,
    )?;
    crate::speech_api::result(&*shared.lock().map_err(invalid)?, &args.job_id)
}
fn conflict() -> ApiError {
    ApiError::new(
        ErrorCode::Conflict,
        "音声要求の対象・版・固定入力が一致しません",
    )
}
fn error(e: TransportError) -> ApiError {
    ApiError::new(
        if e == TransportError::Cancelled {
            ErrorCode::Cancelled
        } else {
            ErrorCode::EngineUnavailable
        },
        format!("ローカル音声: {e}"),
    )
}
pub(crate) async fn run(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    cancel: &AtomicBool,
) -> Result<(), ApiError> {
    let reserved =
        crate::speech_dispatch::Reservation::acquire(&mut *shared.lock().map_err(invalid)?, id)?;
    let Input::Local(input) = reserved.input() else {
        return Err(invalid("音声エンジンが一致しません"));
    };
    crate::http_engine::ensure(shared, crate::http_engine::Engine::local(input.prepared().map_err(invalid)?.settings().engine()), input.endpoint(), cancel).await?;
    let prepared = input.prepared().map_err(invalid)?;
    let client = LocalSpeechClient::new(
        input.endpoint(),
        prepared.settings().engine(),
        Duration::from_secs(300),
    )
    .map_err(error)?;
    tokio::time::timeout(Duration::from_secs(15), client.verify(&prepared, cancel))
        .await
        .map_err(|_| {
            ApiError::new(
                ErrorCode::EngineUnavailable,
                "音声エンジンの確認が時間内に終わりませんでした",
            )
        })?
        .map_err(error)?;
    if cancel.load(Ordering::Acquire) {
        return Err(error(TransportError::Cancelled));
    }
    let dispatched = reserved.begin(&mut *shared.lock().map_err(invalid)?)?;
    let result = client.synthesize(&prepared, cancel).await;
    if result
        .as_ref()
        .err()
        .is_some_and(TransportError::requires_engine_settlement)
    {
        drop(dispatched);
    } else {
        dispatched.confirm_finished(&mut *shared.lock().map_err(invalid)?)?;
    }
    let bytes = result.map_err(error)?;
    crate::speech_output::complete(shared, id, SpeechResponse::local(&prepared, bytes), cancel)
        .await
}
