use std::{fs, os::unix::fs::PermissionsExt, time::Duration};
use sungeki_protocol::{framing::*, *};
use tokio::net::UnixListener;
#[tokio::test]
async fn build_start_wait_exceeds_normal_request_timeout() {
    let recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    let args = serde_json::from_value(serde_json::json!({"job_id":"build", "recipe":recipe, "project_path":"/tmp/new.vvm.json", "wait_ms":3000})).unwrap();
    slow_response(
        SessionRequest::BuildVideo {
            request: BuildVideoRequest::Start { args },
        },
        "build-start",
    )
    .await;
}

#[tokio::test]
async fn build_wait_exceeds_normal_request_timeout() {
    slow_response(
        SessionRequest::BuildVideo {
            request: BuildVideoRequest::Get {
                job_id: "build".into(),
                wait_ms: 3_000,
            },
        },
        "build",
    )
    .await;
}

#[tokio::test]
async fn render_wait_exceeds_normal_request_timeout() {
    slow_response(
        SessionRequest::RenderJob {
            request: RenderJobRequest::Get {
                job_id: "render".into(),
                wait_ms: 3_000,
            },
        },
        "render",
    )
    .await;
}

#[tokio::test]
async fn audio_probe_waits_for_decode_beyond_normal_request_timeout() {
    slow_response(
        SessionRequest::ProbeAudio {
            path: "/tmp/audio.mp3".into(),
        },
        "audio",
    )
    .await;
}
#[tokio::test]
async fn video_preview_waits_for_decode_beyond_normal_request_timeout() {
    let request = serde_json::from_value(serde_json::json!({"type":"video_preview", "args":{
        "session_id":"s", "expected_revision":0, "clip_id":"c", "time_us":0, "proxy_job_id":"job"
    }}))
    .unwrap();
    slow_response(request, "video").await;
}
pub(super) async fn slow_response(request: SessionRequest, label: &str) {
    let directory = std::env::temp_dir().join(format!("wait-{label}-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let listener = UnixListener::bind(directory.join(SOCKET_NAME)).unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let hello: Request = read_frame(&mut socket).await.unwrap();
        write_frame(
            &mut socket,
            &Response {
                request_id: hello.request_id,
                body: ResponseBody::Hello {
                    host: HostInfo {
                        application: APPLICATION.into(),
                        protocol_major: PROTOCOL_MAJOR,
                        protocol_minor: PROTOCOL_MINOR,
                        build_id: BUILD_ID.into(),
                        host_instance_id: "probe-test".into(),
                        capabilities: Capabilities {
                            editing: true,
                            persistence: true,
                            speech: false,
                            rendering: false,
                            vrm: false,
                        },
                    },
                },
            },
        )
        .await
        .unwrap();
        let request: Request = read_frame(&mut socket).await.unwrap();
        tokio::time::sleep(Duration::from_millis(2200)).await;
        write_frame(
            &mut socket,
            &Response {
                request_id: request.request_id,
                body: ResponseBody::Error {
                    error: ApiError::new(ErrorCode::ValidationFailed, "decode finished"),
                },
            },
        )
        .await
        .unwrap();
    });
    let options = crate::HostOptions {
        runtime_dir: directory.clone(),
        host_binary: directory.join("unused"),
    };
    let mut client = crate::HostClient::connect_or_start(&options, "probe-test")
        .await
        .unwrap();
    let result = client.session(request).await;
    server.await.unwrap();
    fs::remove_dir_all(directory).unwrap();
    let error = result.unwrap_err();
    assert_eq!(error.code, ErrorCode::ValidationFailed);
    assert_eq!(error.message, "decode finished");
}

#[tokio::test]
async fn render_start_wait_exceeds_normal_request_timeout() {
    for preview in [false, true] {
        let args = StartRenderArgs {
            session_id: "session".into(),
            job_id: "render".into(),
            expected_revision: 0,
            wait_ms: Some(3_000),
        };
        slow_response(
            SessionRequest::RenderJob {
                request: if preview {
                    RenderJobRequest::Preview { args }
                } else {
                    RenderJobRequest::Start { args }
                },
            },
            if preview {
                "preview-start"
            } else {
                "render-start"
            },
        )
        .await;
    }
}

#[tokio::test]
async fn speech_wait_exceeds_normal_request_timeout() {
    slow_response(
        SessionRequest::SpeechJob {
            request: SpeechJobRequest::Get {
                job_id: "speech".into(),
                wait_ms: 3_000,
            },
        },
        "speech",
    )
    .await;
}

#[tokio::test]
async fn speech_start_wait_exceeds_normal_request_timeout() {
    let request = serde_json::from_value(serde_json::json!({"type":"speech_job","request":{"type":"start","args":{
        "session_id":"s","job_id":"j","expected_revision":0,"clip_id":"c","base_url":"http://127.0.0.1:1","engine_version":"1","wait_ms":3000
    }}})).unwrap();
    slow_response(request, "speech-start").await;
}

#[tokio::test]
async fn audio_preview_wait_exceeds_normal_request_timeout() {
    for start in [false, true] {
        let request = if start {
            AudioPreviewRequest::Start {
                args: StartAudioPreviewArgs {
                    session_id: "s".into(),
                    job_id: "j".into(),
                    expected_revision: 0,
                    wait_ms: Some(3000),
                },
            }
        } else {
            AudioPreviewRequest::Get {
                job_id: "j".into(),
                wait_ms: 3000,
            }
        };
        slow_response(
            SessionRequest::AudioPreview { request },
            if start {
                "audio-start-wait"
            } else {
                "audio-get-wait"
            },
        )
        .await;
    }
}

#[tokio::test]
async fn loudness_wait_exceeds_normal_request_timeout() {
    for start in [false, true] {
        let request = if start {
            LoudnessJobRequest::Start {
                args: StartLoudnessArgs {
                    session_id: "s".into(),
                    job_id: "j".into(),
                    expected_revision: 0,
                    asset_id: "a".into(),
                    wait_ms: Some(3000),
                },
            }
        } else {
            LoudnessJobRequest::Get {
                job_id: "j".into(),
                wait_ms: 3000,
            }
        };
        slow_response(
            SessionRequest::LoudnessJob { request },
            if start {
                "loudness-start-wait"
            } else {
                "loudness-get-wait"
            },
        )
        .await;
    }
}
#[tokio::test]
async fn proxy_wait_exceeds_normal_request_timeout() {
    for start in [false, true] {
        let request = if start {
            ProxyRequest::Start {
                args: GenerateProxiesArgs {
                    session_id: "s".into(),
                    job_id: "j".into(),
                    expected_revision: 0,
                    asset_ids: vec!["a".into()],
                    preset: ProxyPreset::Compact,
                    wait_ms: Some(3000),
                },
            }
        } else {
            ProxyRequest::Get {
                job_id: "j".into(),
                wait_ms: 3000,
            }
        };
        slow_response(
            SessionRequest::Proxy { request },
            if start {
                "proxy-start-wait"
            } else {
                "proxy-get-wait"
            },
        )
        .await;
    }
}
#[tokio::test]
async fn collection_wait_exceeds_normal_request_timeout() {
    for start in [false, true] {
        let request = if start {
            CollectionRequest::Start {
                args: CollectProjectArgs {
                    session_id: "s".into(),
                    job_id: "j".into(),
                    expected_revision: 0,
                    destination: "/tmp/output".into(),
                    dry_run: false,
                    include_checkpoints: false,
                    wait_ms: Some(3000),
                },
            }
        } else {
            CollectionRequest::Get {
                job_id: "j".into(),
                wait_ms: 3000,
            }
        };
        slow_response(
            SessionRequest::Collection { request },
            if start {
                "collection-start-wait"
            } else {
                "collection-get-wait"
            },
        )
        .await;
    }
}
