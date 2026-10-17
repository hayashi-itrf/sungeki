mod sbv2_target_support;
use sbv2_target_support::{environment, id, project};
use std::{sync::atomic::AtomicBool, time::Duration};
use sungeki_speech::sbv2::{Error, ExchangeStage, PreparedSbv2Clip, Sbv2Client};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

async fn server(replies: Vec<Option<Vec<u8>>>) -> (String, tokio::task::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let mut posts = 0;
        for body in replies {
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|s| s == b"\r\n\r\n") {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                request.extend_from_slice(&chunk[..n]);
            }
            if request.starts_with(b"POST /voice?") {
                posts += 1;
            } else {
                assert!(request.starts_with(b"GET /models/info "));
            }
            if let Some(body) = body {
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
                stream.write_all(&body).await.unwrap();
            } else {
                // Drop the connection after receiving the synthesis request.
                // There is no evidence that engine computation has stopped.
                break;
            }
        }
        posts
    });
    (base, handle)
}
#[tokio::test]
async fn distinguishes_unsent_uncertain_and_drained_failures() {
    let (catalog, config, identity) = environment();
    let prepared =
        PreparedSbv2Clip::new(&project(&identity), &id(4), &catalog, &config, &identity).unwrap();
    let original = serde_json::to_vec(catalog.models()).unwrap();
    let mut changed = serde_json::to_value(catalog.models()).unwrap();
    changed["0"]["model_path"] = serde_json::json!("/different");
    let changed = serde_json::to_vec(&changed).unwrap();
    for (replies, stage, uncertain, posts) in [
        (
            vec![Some(changed.clone())],
            ExchangeStage::BeforeSynthesis,
            false,
            0,
        ),
        (
            vec![Some(original.clone()), None],
            ExchangeStage::Synthesis,
            true,
            1,
        ),
        (
            vec![Some(original.clone()), Some(b"wav".to_vec()), Some(changed)],
            ExchangeStage::AfterSynthesis,
            false,
            1,
        ),
    ] {
        let (base, server) = server(replies).await;
        let failure = Sbv2Client::new(&base, Duration::from_secs(1))
            .unwrap()
            .exchange_prepared(prepared.audio(), &AtomicBool::new(false))
            .await
            .unwrap_err();
        assert_eq!(failure.stage, stage);
        assert_eq!(failure.requires_engine_settlement(), uncertain);
        assert_eq!(server.await.unwrap(), posts);
    }
}
#[tokio::test]
async fn completed_exchange_returns_bytes_but_does_not_claim_valid_audio() {
    let (catalog, config, identity) = environment();
    let prepared =
        PreparedSbv2Clip::new(&project(&identity), &id(4), &catalog, &config, &identity).unwrap();
    let metadata = serde_json::to_vec(catalog.models()).unwrap();
    let (base, server) = server(vec![
        Some(metadata.clone()),
        Some(b"not-wav".to_vec()),
        Some(metadata),
    ])
    .await;
    let client = Sbv2Client::new(&base, Duration::from_secs(1)).unwrap();
    let bytes = client
        .exchange_prepared(prepared.audio(), &AtomicBool::new(false))
        .await
        .unwrap();
    assert_eq!(bytes, b"not-wav");
    assert!(prepared.audio().verify_audio(bytes).is_err());
    assert_eq!(server.await.unwrap(), 1);
    let failure = client
        .exchange_prepared(prepared.audio(), &AtomicBool::new(true))
        .await
        .unwrap_err();
    assert_eq!(failure.stage, ExchangeStage::BeforeSynthesis);
    assert_eq!(failure.error, Error::Cancelled);
    assert!(!failure.requires_engine_settlement());
}
