use std::time::Duration;
use sungeki_protocol::*;

/// IPC response allowance, separate from each job's bounded wait.
pub(super) fn for_request(body: &RequestBody) -> Duration {
    if let RequestBody::Session {
        request: SessionRequest::SpeechJob { request },
    } = body
    {
        let start = match request {
            SpeechJobRequest::StartLocal { args } | SpeechJobRequest::Start { args } => args.wait_ms.map(|ms| (ms, 2_000)),
            SpeechJobRequest::StartSbv2 { args } => args.wait_ms.map(|ms| (ms, 600_000)),
            SpeechJobRequest::StartQwen { args } => args.wait_ms.map(|ms| (ms, 600_000)),
            SpeechJobRequest::StartIrodori { args } => args.wait_ms.map(|ms| (ms, 600_000)),
            _ => None,
        };
        if let Some((ms, preparation)) = start {
            return Duration::from_millis(u64::from(ms) + preparation);
        }
    }
    match body {
        RequestBody::Session {
            request: SessionRequest::InstallSpeechConfig { .. },
        } => Duration::from_secs(600),
        // Profile lists read potentially large inventories; details also
        // hash local model/runtime originals before replying.
        // Real SSD inventories take minutes; the normal IPC timeout would
        // discard a valid profile or leave a started job's response uncertain.
        RequestBody::Session {
            request:
                SessionRequest::SpeechJob {
                    request:
                        sungeki_protocol::SpeechJobRequest::ListIrodoriProfiles
                        | sungeki_protocol::SpeechJobRequest::ListQwenProfiles
                        | sungeki_protocol::SpeechJobRequest::ListSbv2Profiles
                        | sungeki_protocol::SpeechJobRequest::GetIrodoriProfile { .. }
                        | sungeki_protocol::SpeechJobRequest::StartIrodori { .. }
                        | sungeki_protocol::SpeechJobRequest::GetQwenProfile { .. }
                        | sungeki_protocol::SpeechJobRequest::StartQwen { .. }
                        | sungeki_protocol::SpeechJobRequest::GetSbv2Profile { .. }
                        | sungeki_protocol::SpeechJobRequest::StartSbv2 { .. },
                },
        } => Duration::from_secs(600),
        RequestBody::Session {
            request:
                SessionRequest::AnalyzeAudio { .. }
                | SessionRequest::ProbeAudio { .. }
                | SessionRequest::ProbeVideo { .. }
                | SessionRequest::VideoPreview { .. },
        } => Duration::from_secs(65),
        RequestBody::Session {
            request:
                SessionRequest::RenderJob {
                    request:
                        sungeki_protocol::RenderJobRequest::Publish { .. }
                        | sungeki_protocol::RenderJobRequest::Frame { .. },
                },
        } => Duration::from_secs(65),
        RequestBody::Session {
            request: SessionRequest::ProbePsd { .. } | SessionRequest::PsdPreview { .. },
        } => Duration::from_secs(65),
        RequestBody::Session {
            request:
                SessionRequest::GetLocalSpeechCatalog { .. } | SessionRequest::GetLocalSpeechPolicy { .. } | SessionRequest::GetVoicevoxSpeaker { .. }
                | SessionRequest::DiscoverVoicevox { .. }
                | SessionRequest::GetVoicevoxCatalog { .. },
        } => Duration::from_secs(620),
        RequestBody::Session {
            request:
                SessionRequest::GetReviewJob { wait_ms, .. }
                | SessionRequest::StartReview { wait_ms, .. },
        }
        | RequestBody::Session {
            request:
                SessionRequest::RenderJob {
                    request: RenderJobRequest::Get { wait_ms, .. },
                },
        }
        | RequestBody::Session {
            request:
                SessionRequest::BuildVideo {
                    request: sungeki_protocol::BuildVideoRequest::Get { wait_ms, .. },
                },
        }
        | RequestBody::Session {
            request:
                SessionRequest::BuildVideo {
                    request:
                        sungeki_protocol::BuildVideoRequest::Start {
                            args:
                                sungeki_protocol::StartBuildVideoArgs {
                                    wait_ms: Some(wait_ms),
                                    ..
                                },
                        },
                },
        }
        | RequestBody::Session {
            request:
                SessionRequest::RenderJob {
                    request:
                        RenderJobRequest::Start {
                            args:
                                sungeki_protocol::StartRenderArgs {
                                    wait_ms: Some(wait_ms),
                                    ..
                                },
                        }
                        | RenderJobRequest::Preview {
                            args:
                                sungeki_protocol::StartRenderArgs {
                                    wait_ms: Some(wait_ms),
                                    ..
                                },
                        },
                },
        }
        | RequestBody::Session {
            request:
                SessionRequest::SpeechJob {
                    request: SpeechJobRequest::Get { wait_ms, .. },
                },
        }
        | RequestBody::Session {
            request:
                SessionRequest::AudioPreview {
                    request:
                        AudioPreviewRequest::Get { wait_ms, .. }
                        | AudioPreviewRequest::Start {
                            args:
                                StartAudioPreviewArgs {
                                    wait_ms: Some(wait_ms),
                                    ..
                                },
                        },
                },
        }
        | RequestBody::Session {
            request:
                SessionRequest::LoudnessJob {
                    request:
                        LoudnessJobRequest::Get { wait_ms, .. }
                        | LoudnessJobRequest::Start {
                            args:
                                StartLoudnessArgs {
                                    wait_ms: Some(wait_ms),
                                    ..
                                },
                        },
                },
        }
        | RequestBody::Session {
            request:
                SessionRequest::Proxy {
                    request:
                        ProxyRequest::Get { wait_ms, .. }
                        | ProxyRequest::Start {
                            args:
                                GenerateProxiesArgs {
                                    wait_ms: Some(wait_ms),
                                    ..
                                },
                        },
                },
        }
        | RequestBody::Session {
            request:
                SessionRequest::Collection {
                    request:
                        CollectionRequest::Get { wait_ms, .. }
                        | CollectionRequest::Start {
                            args:
                                CollectProjectArgs {
                                    wait_ms: Some(wait_ms),
                                    ..
                                },
                        },
                },
        }
        | RequestBody::Watch { wait_ms, .. } => Duration::from_millis(u64::from(*wait_ms) + 2_000),
        RequestBody::Session {
            request:
                SessionRequest::Open {
                    import_checkpoints: true,
                    ..
                },
        } => Duration::from_secs(65),
        RequestBody::Session {
            request: SessionRequest::Save { .. } | SessionRequest::Open { .. },
        } => Duration::from_secs(30),
        _ => Duration::from_secs(2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn speech_wait_preserves_profile_preparation_allowance() {
        for kind in ["start", "start_local", "start_sbv2", "start_qwen", "start_irodori"] {
            for wait in [None, Some(0u32), Some(30_000)] {
                let mut args = serde_json::json!({"session_id":"s","job_id":"j","clip_id":"c","expected_revision":0});
                if matches!(kind, "start" | "start_local") {
                    args["base_url"] = "http://127.0.0.1:1".into();
                    args["engine_version"] = "1".into();
                } else {
                    args["profile_id"] = "local".into();
                }
                if let Some(wait) = wait {
                    args["wait_ms"] = wait.into();
                }
                let request = serde_json::from_value(
                    serde_json::json!({"type":"speech_job","request":{"type":kind,"args":args}}),
                )
                .unwrap();
                let body = RequestBody::Session { request };
                let preparation = if matches!(kind, "start" | "start_local") { 2_000 } else { 600_000 };
                assert_eq!(
                    for_request(&body),
                    Duration::from_millis(preparation + u64::from(wait.unwrap_or(0)))
                );
            }
        }
    }
}
