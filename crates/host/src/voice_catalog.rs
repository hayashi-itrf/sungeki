//! Catalog I/O with optional configured engine startup.
use std::{sync::atomic::AtomicBool, time::Duration};
use sungeki_protocol::*;
use sungeki_speech::voicevox::{EngineInfo, TransportError, VoicevoxDiscovery};
use tokio::sync::Semaphore;

static REQUESTS: Semaphore = Semaphore::const_new(2);
pub fn handles(request: &SessionRequest) -> bool {
    matches!(
        request,
        SessionRequest::GetLocalSpeechPolicy { .. } | SessionRequest::GetLocalSpeechCatalog { .. }
            | SessionRequest::GetVoicevoxSpeaker { .. }
            | SessionRequest::DiscoverVoicevox { .. }
            | SessionRequest::GetVoicevoxCatalog { .. }
    )
}
pub async fn execute(shared: &std::sync::Arc<std::sync::Mutex<crate::sessions::Sessions>>, request: SessionRequest) -> Result<SessionResponse, ApiError> {
    let _permit = REQUESTS.try_acquire().map_err(|_| {
        let mut error = ApiError::new(
            ErrorCode::EngineUnavailable,
            "話者一覧を取得中です。少し待って再取得してください",
        );
        error.retryable = true;
        error
    })?;
    let launch = match &request {
        SessionRequest::GetLocalSpeechCatalog { args } => Some((crate::http_engine::Engine::local(args.engine), args.base_url.as_str())),
        SessionRequest::DiscoverVoicevox { args } => Some((crate::http_engine::Engine::Voicevox, args.base_url.as_str())),
        SessionRequest::GetVoicevoxCatalog { args } => Some((crate::http_engine::Engine::Voicevox, args.base_url.as_str())),
        _ => None,
    };
    if let Some((engine, endpoint)) = launch {crate::http_engine::ensure(shared, engine, endpoint, &AtomicBool::new(false)).await?;}
    // Read-only requests may safely be dropped at this whole-operation deadline.
    // This must not be reused as a cancellation strategy for synthesis.
    tokio::time::timeout(Duration::from_secs(15), fetch(request))
        .await
        .map_err(|_| {
            ApiError::new(
                ErrorCode::EngineUnavailable,
                "話者一覧の取得が15秒以内に完了しませんでした",
            )
        })?
}
async fn fetch(request: SessionRequest) -> Result<SessionResponse, ApiError> {
    let cancelled = AtomicBool::new(false);
    match request {
        SessionRequest::GetLocalSpeechPolicy { args } => {
            let client = sungeki_speech::local_http::LocalSpeechClient::new(&args.base_url,args.engine,Duration::from_secs(5)).map_err(error)?;
            Ok(SessionResponse::LocalSpeechPolicy { policy:client.policy(&args.speaker_uuid,&cancelled).await.map_err(error)? })
        }

        SessionRequest::GetLocalSpeechCatalog { args } => {
            let client = sungeki_speech::local_http::LocalSpeechClient::new(
                &args.base_url,
                args.engine,
                Duration::from_secs(5),
            )
            .map_err(error)?;
            let result = client.catalog(&cancelled).await.map_err(error)?;
            Ok(SessionResponse::LocalSpeechCatalog {
                catalog: LocalSpeechCatalog {
                    engine: args.engine,
                    version: result.version,
                    speakers: result
                        .speakers
                        .into_iter()
                        .map(|s| LocalSpeechSpeaker {
                            speaker_uuid: s.speaker_uuid,
                            name: s.name,
                            styles: s
                                .styles
                                .into_iter()
                                .map(|s| LocalSpeechStyle {
                                    id: s.id,
                                    name: s.name,
                                })
                                .collect(),
                        })
                        .collect(),
                },
            })
        }

        SessionRequest::GetVoicevoxSpeaker { args } => {
            let result = client(&args.base_url)?
                .speaker_details(
                    &expected(args.expected_engine),
                    &args.core_version,
                    &args.speaker_uuid,
                    &cancelled,
                )
                .await
                .map_err(error)?;
            Ok(SessionResponse::VoicevoxSpeaker {
                details: VoicevoxSpeakerDetails {
                    engine: convert(result.engine),
                    core_version: result.core_version,
                    speaker: speaker(result.speaker),
                    engine_terms: result.engine_terms,
                    speaker_policy: result.speaker_policy,
                    default_sampling_rate: result.default_sampling_rate,
                },
            })
        }
        SessionRequest::DiscoverVoicevox { args } => {
            let client = client(&args.base_url)?;
            Ok(SessionResponse::VoicevoxEngine {
                engine: convert(client.engine(&cancelled).await.map_err(error)?),
            })
        }
        SessionRequest::GetVoicevoxCatalog { args } => {
            let client = client(&args.base_url)?;
            let expected = expected(args.expected_engine);
            let catalog = client
                .catalog(&expected, &args.core_version, &cancelled)
                .await
                .map_err(error)?;
            Ok(SessionResponse::VoicevoxCatalog {
                engine: convert(catalog.engine),
                core_version: catalog.core_version,
                speakers: catalog.speakers.into_iter().map(speaker).collect(),
            })
        }
        _ => Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "話者一覧以外の要求です",
        )),
    }
}
fn client(base: &str) -> Result<VoicevoxDiscovery, ApiError> {
    if base.len() > 2048 {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "エンジンURLが長すぎます",
        ));
    }
    VoicevoxDiscovery::new(base, Duration::from_secs(3)).map_err(error)
}
fn convert(info: EngineInfo) -> VoicevoxEngineInfo {
    VoicevoxEngineInfo {
        engine_id: info.engine_id,
        name: info.name,
        version: info.version,
        core_versions: info.core_versions,
    }
}
fn error(error: TransportError) -> ApiError {
    let code = match &error {
        TransportError::Configuration(_) | TransportError::Input(_) => ErrorCode::ValidationFailed,
        TransportError::Cancelled => ErrorCode::Cancelled,
        _ => ErrorCode::EngineUnavailable,
    };
    ApiError::new(code, error.to_string())
}

fn expected(info: VoicevoxEngineInfo) -> EngineInfo {
    EngineInfo {
        engine_id: info.engine_id,
        name: info.name,
        version: info.version,
        core_versions: info.core_versions,
    }
}
fn speaker(value: sungeki_speech::voicevox::Speaker) -> VoicevoxSpeakerInfo {
    VoicevoxSpeakerInfo {
        speaker_uuid: value.speaker_uuid,
        name: value.name,
        version: value.version,
        styles: value
            .styles
            .into_iter()
            .map(|style| VoicevoxStyleInfo {
                id: style.id,
                name: style.name,
                kind: style.kind,
            })
            .collect(),
    }
}
