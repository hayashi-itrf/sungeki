#[allow(dead_code)]
mod http_support;
use http_support::{Reply, server_with_method};
use serde_json::{Value, json};
use std::{sync::atomic::AtomicBool, time::Duration};
use sungeki_speech::voicevox::{EngineInfo, Stage, TransportError, VoicevoxDiscovery};
const ID: &str = "074fc39e-678b-4c13-8916-ffca8d505d1d";
fn info() -> EngineInfo {
    EngineInfo {
        engine_id: ID.into(),
        name: "テストエンジン".into(),
        version: "1.0".into(),
        core_versions: vec!["0.16.0".into()],
    }
}
fn reply(value: Value) -> Reply {
    Reply::ok(serde_json::to_vec(&value).unwrap())
}
fn metadata(version: &str) -> Vec<Reply> {
    vec![
        reply(json!({"uuid": ID,"name":"テストエンジン","ignored":"extension"})),
        reply(json!(version)),
        reply(json!(["0.16.0"])),
    ]
}
fn speakers() -> Value {
    json!([{"speaker_uuid":ID,"name":"話者","version":"1","styles":[
        {"id":1,"name":"通常"}, {"id":2,"name":"歌唱","type":"sing"},
        {"id":3,"name":"ストリーム","type":"streaming_talk"},
        {"id":4,"name":"将来の種類","type":"unknown_future"}]}])
}
#[tokio::test]
async fn discovers_identity_and_explicit_core_talk_styles_without_synthesis() {
    let mut replies = metadata("1.0");
    replies.extend(metadata("1.0"));
    replies.push(reply(speakers()));
    replies.extend(metadata("1.0"));
    let (base, task) = server_with_method(replies, "GET").await;
    let client = VoicevoxDiscovery::new(&base, Duration::from_secs(2)).unwrap();
    let cancel = AtomicBool::new(false);
    let engine = client.engine(&cancel).await.unwrap();
    assert_eq!(engine, info());
    let catalog = client.catalog(&engine, "0.16.0", &cancel).await.unwrap();
    assert_eq!(
        catalog.speakers[0]
            .styles
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    let requests = task.await.unwrap();
    assert_eq!(requests.len(), 10);
    assert!(requests.iter().all(|r| r.body.is_empty()));
    assert_eq!(requests[6].url.path(), "/speakers");
    assert_eq!(requests[6].url.query(), Some("core_version=0.16.0"));
}
#[tokio::test]
async fn changed_engine_rejects_catalog_before_and_after_speakers() {
    for after in [false, true] {
        let mut replies = metadata(if after { "1.0" } else { "2.0" });
        if after {
            replies.push(reply(speakers()));
            replies.extend(metadata("2.0"));
        }
        let (base, task) = server_with_method(replies, "GET").await;
        let client = VoicevoxDiscovery::new(&base, Duration::from_secs(2)).unwrap();
        assert!(matches!(
            client
                .catalog(&info(), "0.16.0", &AtomicBool::new(false))
                .await,
            Err(TransportError::Metadata(_))
        ));
        assert_eq!(task.await.unwrap().len(), if after { 7 } else { 3 });
    }
}
#[tokio::test]
async fn malformed_and_duplicate_speakers_fail_closed() {
    let mut duplicate = speakers();
    duplicate[0]["styles"][1]["id"] = json!(1);
    let mut invalid = speakers();
    invalid[0]["speaker_uuid"] = json!("invalid");
    let mut missing = speakers();
    missing[0].as_object_mut().unwrap().remove("version");
    let mut duplicate_speaker = speakers();
    let mut second = duplicate_speaker[0].clone();
    second["speaker_uuid"] = json!(ID.to_uppercase());
    second["styles"] = json!([]);
    duplicate_speaker.as_array_mut().unwrap().push(second);
    for body in [duplicate, invalid, missing, duplicate_speaker] {
        let mut replies = metadata("1.0");
        replies.push(reply(body));
        let (base, task) = server_with_method(replies, "GET").await;
        let client = VoicevoxDiscovery::new(&base, Duration::from_secs(2)).unwrap();
        assert!(matches!(
            client
                .catalog(&info(), "0.16.0", &AtomicBool::new(false))
                .await,
            Err(TransportError::Metadata(_))
        ));
        assert_eq!(task.await.unwrap().len(), 4);
    }
}
#[tokio::test]
async fn cancellation_unknown_core_and_remote_urls_do_not_send() {
    assert!(VoicevoxDiscovery::new("https://example.com", Duration::from_secs(1)).is_err());
    let client = VoicevoxDiscovery::new("http://127.0.0.1:1", Duration::from_secs(1)).unwrap();
    assert!(matches!(
        client.engine(&AtomicBool::new(true)).await,
        Err(TransportError::Cancelled)
    ));
    assert!(matches!(
        client
            .catalog(&info(), "missing", &AtomicBool::new(false))
            .await,
        Err(TransportError::Metadata(_))
    ));
}
#[tokio::test]
async fn metadata_http_failures_are_bounded_and_do_not_require_synthesis_settlement() {
    let replies = [
        Reply {
            header:
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1\r\nContent-Length: 0\r\n\r\n"
                    .into(),
            ..Reply::ok(vec![])
        },
        Reply {
            header: "HTTP/1.1 200 OK\r\nContent-Length: 8388609\r\n\r\n".into(),
            ..Reply::ok(vec![])
        },
        Reply {
            body_delay: Duration::from_millis(100),
            ..reply(json!({"uuid":ID,"name":"エンジン"}))
        },
    ];
    for (i, reply) in replies.into_iter().enumerate() {
        let (base, task) = server_with_method(vec![reply], "GET").await;
        let client = VoicevoxDiscovery::new(&base, Duration::from_millis(50)).unwrap();
        let err = client.engine(&AtomicBool::new(false)).await.unwrap_err();
        match i {
            0 => assert!(matches!(
                err,
                TransportError::Http {
                    stage: Stage::Catalog,
                    status: 302
                }
            )),
            1 => assert!(matches!(
                err,
                TransportError::TooLarge {
                    stage: Stage::Catalog,
                    ..
                }
            )),
            _ => assert!(matches!(err, TransportError::Timeout(Stage::Catalog))),
        }
        assert!(!err.requires_engine_settlement());
        assert_eq!(task.await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn invalid_core_lists_and_interrupted_discovery_never_return_partial_metadata() {
    for cores in [
        json!([]),
        json!(["0.16.0", "0.16.0"]),
        json!([" "]),
        json!(["bad\ncore"]),
    ] {
        let mut replies = metadata("1.0");
        replies[2] = reply(cores);
        let (base, task) = server_with_method(replies, "GET").await;
        let client = VoicevoxDiscovery::new(&base, Duration::from_secs(2)).unwrap();
        assert!(matches!(
            client.engine(&AtomicBool::new(false)).await,
            Err(TransportError::Metadata(_))
        ));
        assert_eq!(task.await.unwrap().len(), 3);
    }
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let mut response = metadata("1.0").remove(0);
    response.cancel = Some(cancelled.clone());
    let (base, task) = server_with_method(vec![response], "GET").await;
    let client = VoicevoxDiscovery::new(&base, Duration::from_secs(2)).unwrap();
    assert!(matches!(
        client.engine(&cancelled).await,
        Err(TransportError::Cancelled)
    ));
    assert_eq!(task.await.unwrap().len(), 1);
}

fn catalog_replies() -> Vec<Reply> {
    let mut result = metadata("1.0");
    result.push(reply(speakers()));
    result.extend(metadata("1.0"));
    result
}
#[tokio::test]
async fn speaker_details_preserve_terms_and_verify_selected_voice() {
    let mut replies = catalog_replies();
    replies.push(reply(json!({"uuid":ID,"name":"テストエンジン","terms_of_service":"条件\n原文","default_sampling_rate":24000})));
    replies.push(reply(json!({"policy":"話者の条件\n<script>text only</script>","portrait":"https://example.com/do-not-fetch"})));
    replies.extend(catalog_replies());
    let (base, task) = server_with_method(replies, "GET").await;
    let client = VoicevoxDiscovery::new(&base, Duration::from_secs(2)).unwrap();
    let details = client
        .speaker_details(&info(), "0.16.0", ID, &AtomicBool::new(false))
        .await
        .unwrap();
    assert_eq!(details.default_sampling_rate, 24000);
    assert_eq!(details.engine_terms, "条件\n原文");
    assert_eq!(
        details.speaker_policy,
        "話者の条件\n<script>text only</script>"
    );
    let requests = task.await.unwrap();
    assert_eq!(requests.len(), 16);
    let pairs: std::collections::HashMap<_, _> = requests[8].url.query_pairs().collect();
    assert_eq!(pairs["speaker_uuid"], ID);
    assert_eq!(pairs["resource_format"], "url");
    assert_eq!(pairs["core_version"], "0.16.0");
}
#[tokio::test]
async fn changed_speaker_and_invalid_policy_cannot_be_used_for_registration() {
    for changed in [false, true] {
        let mut replies = catalog_replies();
        replies.push(reply(json!({"uuid":ID,"name":"テストエンジン","terms_of_service":"条件","default_sampling_rate":24000})));
        replies.push(reply(
            json!({"policy":if changed {"条件"} else {"bad\0policy"}}),
        ));
        if changed {
            replies.extend(metadata("1.0"));
            let mut updated = speakers();
            updated[0]["version"] = json!("2");
            replies.push(reply(updated));
            replies.extend(metadata("1.0"));
        }
        let (base, task) = server_with_method(replies, "GET").await;
        let client = VoicevoxDiscovery::new(&base, Duration::from_secs(2)).unwrap();
        assert!(matches!(
            client
                .speaker_details(&info(), "0.16.0", ID, &AtomicBool::new(false))
                .await,
            Err(TransportError::Metadata(_))
        ));
        assert_eq!(task.await.unwrap().len(), if changed { 16 } else { 9 });
    }
}
