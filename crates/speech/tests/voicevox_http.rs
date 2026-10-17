mod http_support;
use http_support::{Reply, query, server, settings};
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use sungeki_speech::voicevox::{Stage, TransportError, VoicevoxClient};

fn client(base: &str) -> VoicevoxClient {
    VoicevoxClient::new(base, "0.16.0", Duration::from_secs(2)).unwrap()
}
#[tokio::test]
async fn real_http_preserves_text_and_binds_core_and_voice_across_stages() {
    let (base, server) = server(vec![
        Reply::ok(query()),
        Reply::ok(b"unverified audio".to_vec()),
    ])
    .await;
    let text = "ア & ? + # /\n日本語";
    let response = client(&base)
        .synthesize(text, &settings(), &AtomicBool::new(false))
        .await
        .unwrap();
    assert_eq!(response.unverified_wav, b"unverified audio");
    assert_eq!(response.query.speed_scale, 1.2);
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 2);
    for (request, path) in requests.iter().zip(["/audio_query", "/synthesis"]) {
        assert_eq!(request.url.path(), path);
        let args: BTreeMap<_, _> = request.url.query_pairs().collect();
        assert_eq!(args["speaker"], "42");
        assert_eq!(args["core_version"], "0.16.0");
    }
    let first: BTreeMap<_, _> = requests[0].url.query_pairs().collect();
    assert_eq!(first["text"], text);
    assert_eq!(first["enable_katakana_english"], "true");
    assert!(requests[0].body.is_empty());
    let second: BTreeMap<_, _> = requests[1].url.query_pairs().collect();
    assert_eq!(second["enable_interrogative_upspeak"], "false");
    assert!(!second.contains_key("text"));
    let body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(body["speedScale"], 1.2);
    assert_eq!(body["outputSamplingRate"], 48000);
}
#[tokio::test]
async fn cancellation_drains_current_stage_and_never_returns_its_result() {
    for during_synthesis in [false, true] {
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut reply = Reply::ok(if during_synthesis {
            b"audio".to_vec()
        } else {
            query()
        });
        reply.cancel = Some(cancelled.clone());
        reply.body_delay = Duration::from_millis(20);
        let replies = if during_synthesis {
            vec![Reply::ok(query()), reply]
        } else {
            vec![reply]
        };
        let (base, server) = server(replies).await;
        let result = client(&base)
            .synthesize("ア", &settings(), &cancelled)
            .await;
        assert!(matches!(result, Err(TransportError::Cancelled)));
        let requests = server.await.unwrap();
        assert_eq!(requests.len(), if during_synthesis { 2 } else { 1 });
    }
    // No reachable engine is necessary when cancellation is already requested.
    let result = client("http://127.0.0.1:1")
        .synthesize("ア", &settings(), &AtomicBool::new(true))
        .await;
    assert!(matches!(result, Err(TransportError::Cancelled)));
}
#[tokio::test]
async fn rejects_redirect_and_engine_error_without_retry_or_forwarding() {
    for code in [302, 422, 503] {
        let reply = Reply {
            header: format!(
                "HTTP/1.1 {code} Failed\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\n\r\n"
            ),
            ..Reply::ok(vec![])
        };
        let (base, server) = server(vec![reply]).await;
        let error = client(&base)
            .synthesize("secret text", &settings(), &AtomicBool::new(false))
            .await
            .err()
            .unwrap();
        assert_eq!(
            error,
            TransportError::Http {
                stage: Stage::Analysis,
                status: code
            }
        );
        assert!(!error.to_string().contains("secret text"));
        assert_eq!(server.await.unwrap().len(), 1);
    }
}
#[tokio::test]
async fn deadline_covers_body_read_and_reports_unknown_engine_completion() {
    let reply = Reply {
        body_delay: Duration::from_millis(250),
        ..Reply::ok(query())
    };
    let (base, server) = server(vec![reply]).await;
    let client = VoicevoxClient::new(&base, "0.16.0", Duration::from_millis(100)).unwrap();
    let error = client
        .synthesize("ア", &settings(), &AtomicBool::new(false))
        .await
        .err()
        .unwrap();
    assert_eq!(error, TransportError::Timeout(Stage::Analysis));
    assert!(error.requires_engine_settlement());
    assert_eq!(server.await.unwrap().len(), 1);
}
#[tokio::test]
async fn response_size_is_bounded_with_and_without_content_length() {
    for chunked in [false, true] {
        let reply = if chunked {
            let data = "x".repeat(8 * 1024 * 1024 + 1);
            Reply {
                header: "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".into(),
                body: format!("{:x}\r\n{data}\r\n0\r\n\r\n", data.len()).into_bytes(),
                ..Reply::ok(vec![])
            }
        } else {
            Reply {
                header: "HTTP/1.1 200 OK\r\nContent-Length: 8388609\r\n\r\n".into(),
                ..Reply::ok(vec![])
            }
        };
        let (base, server) = server(vec![reply]).await;
        let error = client(&base)
            .synthesize("ア", &settings(), &AtomicBool::new(false))
            .await
            .err()
            .unwrap();
        assert_eq!(
            error,
            TransportError::TooLarge {
                stage: Stage::Analysis,
                limit: 8 * 1024 * 1024
            }
        );
        assert!(error.requires_engine_settlement());
        assert_eq!(server.await.unwrap().len(), 1);
    }
}
#[test]
fn endpoint_and_deadline_validation_precede_network_access() {
    for base in [
        "http://localhost:50021",
        "http://192.168.0.1:50021",
        "https://127.0.0.1",
        "http://user:pass@127.0.0.1",
        "http://127.0.0.1/path",
        "http://127.0.0.1/?x=1",
        "http://127.0.0.1/#x",
    ] {
        assert!(
            VoicevoxClient::new(base, "0.16.0", Duration::from_secs(1)).is_err(),
            "{base}"
        );
    }
    assert!(VoicevoxClient::new("http://[::1]:50021", "0.16.0", Duration::from_secs(1)).is_ok());
    for duration in [Duration::ZERO, Duration::from_secs(301)] {
        assert!(VoicevoxClient::new("http://127.0.0.1", "0.16.0", duration).is_err());
    }
    assert!(VoicevoxClient::new("http://127.0.0.1", "", Duration::from_secs(1)).is_err());
}

#[tokio::test]
async fn invalid_analysis_never_reaches_synthesis_and_truncated_audio_is_not_success() {
    let (base, mock) = server(vec![Reply::ok(b"{}".to_vec())]).await;
    let error = client(&base)
        .synthesize("ア", &settings(), &AtomicBool::new(false))
        .await
        .err()
        .unwrap();
    assert!(matches!(error, TransportError::Input(_)));
    assert_eq!(mock.await.unwrap().len(), 1);
    let reply = Reply {
        header: "HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n".into(),
        ..Reply::ok(b"short".to_vec())
    };
    let (base, mock) = server(vec![Reply::ok(query()), reply]).await;
    let error = client(&base)
        .synthesize("ア", &settings(), &AtomicBool::new(false))
        .await
        .err()
        .unwrap();
    assert_eq!(error, TransportError::Connection(Stage::Synthesis));
    assert!(error.requires_engine_settlement());
    assert_eq!(mock.await.unwrap().len(), 2);
}

#[tokio::test]
async fn synthesis_has_its_own_audio_limit_and_http_status() {
    for (header, expected) in [
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 67108865\r\n\r\n",
            TransportError::TooLarge {
                stage: Stage::Synthesis,
                limit: 64 * 1024 * 1024,
            },
        ),
        (
            "HTTP/1.1 500 Failed\r\nContent-Length: 0\r\n\r\n",
            TransportError::Http {
                stage: Stage::Synthesis,
                status: 500,
            },
        ),
    ] {
        let reply = Reply {
            header: header.into(),
            ..Reply::ok(vec![])
        };
        let (base, mock) = server(vec![Reply::ok(query()), reply]).await;
        let error = client(&base)
            .synthesize("ア", &settings(), &AtomicBool::new(false))
            .await
            .err()
            .unwrap();
        assert_eq!(error, expected);
        assert!(error.requires_engine_settlement());
        assert_eq!(mock.await.unwrap().len(), 2);
    }
}
