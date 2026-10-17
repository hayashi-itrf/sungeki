#[path = "irodori/common.rs"]
mod common;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_speech::irodori::{
    Catalog, Error, ExchangeStage, IrodoriClient, ModelCapabilities, PreparedIrodoriInput,
    RegisteredVoice, canonical_endpoint,
};

fn metadata(reference: &str) -> serde_json::Value {
    json!({"model":{"checkpoint":"model","speaker_conditioning":true,
        "caption_conditioning":true,"sample_rate":48000,"channels":1},
        "voices":[{"id":"registered","name":"登録話者","reference":reference,"caption":null}]})
}
fn input() -> PreparedIrodoriInput {
    let value = metadata("none");
    let model: ModelCapabilities = serde_json::from_value(value["model"].clone()).unwrap();
    let voices: Vec<RegisteredVoice> = serde_json::from_value(value["voices"].clone()).unwrap();
    let catalog = Catalog::new(model, voices).unwrap();
    let mut source = common::source();
    source.identity.sample_rate = 48000;
    let selected = catalog.select(&source.settings, &source.identity).unwrap();
    PreparedIrodoriInput::from_selected("字幕", Some("よみあげ"), &[], &selected).unwrap()
}
type Response = (u16, &'static str, Vec<u8>);
fn json_response(value: serde_json::Value) -> Response {
    (200, "application/json", serde_json::to_vec(&value).unwrap())
}
fn serve(responses: Vec<Response>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let thread = std::thread::spawn(move || {
        let mut requests = vec![];
        for (status, content_type, body) in responses {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(std::time::Instant::now() < deadline, "request not received");
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = vec![];
            let mut byte = [0];
            while !bytes.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
                assert!(bytes.len() < 8192);
            }
            let headers = String::from_utf8(bytes.clone()).unwrap();
            let size = headers
                .lines()
                .find_map(|line| {
                    line.to_lowercase()
                        .strip_prefix("content-length: ")
                        .map(|n| n.parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            let mut payload = vec![0; size];
            stream.read_exact(&mut payload).unwrap();
            bytes.extend(payload);
            requests.push(String::from_utf8(bytes).unwrap());
            write!(stream, "HTTP/1.1 {status} Response\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            stream.write_all(&body).unwrap();
        }
        requests
    });
    (base, thread)
}
#[test]
fn only_explicit_next_loopback_endpoints_and_bounded_timeout() {
    assert_eq!(
        canonical_endpoint("http://127.0.0.1:50123").unwrap(),
        "http://127.0.0.1:50123/"
    );
    for url in [
        "http://localhost:50123",
        "http://127.0.0.2:50123",
        "http://127.0.0.1",
        "https://127.0.0.1:50123",
        "http://127.0.0.1:50123/path",
        "http://user@127.0.0.1:50123",
        "http://127.0.0.1:50123/?x=1",
    ] {
        assert!(canonical_endpoint(url).is_err(), "{url}");
    }
    assert!(IrodoriClient::new("http://127.0.0.1:50123", Duration::ZERO).is_err());
}
#[tokio::test]
async fn exchange_preserves_request_and_checks_returned_wav() {
    let wav = include_bytes!("../../../tests/fixtures/irodori-float.wav").to_vec();
    let (base, server) = serve(vec![
        json_response(metadata("none")),
        (200, "audio/wav", wav.clone()),
        json_response(metadata("none")),
    ]);
    let client = IrodoriClient::new(&base, Duration::from_secs(2)).unwrap();
    let input = input();
    let output = client
        .exchange_prepared(&input, &AtomicBool::new(false))
        .await
        .unwrap();
    assert_eq!(input.verify_audio(output).unwrap().info().frames, 5);
    let requests = server.join().unwrap();
    assert!(requests[0].starts_with("GET /v1/catalog "));
    assert!(requests[1].starts_with("POST /v1/synthesize "));
    let body: serde_json::Value =
        serde_json::from_str(requests[1].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["text"], "よみあげ");
    assert_eq!(body["settings"]["seed"], 42);
    assert_eq!(body.as_object().unwrap().len(), 2);
}
#[tokio::test]
async fn changed_catalog_and_failed_dispatch_report_the_correct_stage() {
    for mode in 0..3 {
        let responses = match mode {
            0 => vec![json_response(metadata("audio"))],
            1 => vec![
                json_response(metadata("none")),
                (500, "application/json", b"private error".to_vec()),
            ],
            _ => vec![
                json_response(metadata("none")),
                (200, "audio/wav", b"fixture".to_vec()),
                json_response(metadata("latent")),
            ],
        };
        let (base, server) = serve(responses);
        let client = IrodoriClient::new(&base, Duration::from_secs(2)).unwrap();
        let error = client
            .exchange_prepared(&input(), &AtomicBool::new(false))
            .await
            .unwrap_err();
        assert_eq!(
            error.stage,
            [
                ExchangeStage::BeforeSynthesis,
                ExchangeStage::Synthesis,
                ExchangeStage::AfterSynthesis
            ][mode]
        );
        assert_eq!(error.requires_engine_settlement(), mode == 1);
        assert!(!error.to_string().contains("private"));
        server.join().unwrap();
    }
}
#[tokio::test]
async fn cancellation_offline_input_and_bad_content_type_are_rejected() {
    let client = IrodoriClient::new("http://127.0.0.1:50123", Duration::from_secs(2)).unwrap();
    let error = client
        .exchange_prepared(&input(), &AtomicBool::new(true))
        .await
        .unwrap_err();
    assert_eq!(error.error, Error::Cancelled);
    let offline = PreparedIrodoriInput::new("字幕", None, &[], input().audio_source()).unwrap();
    let error = client
        .exchange_prepared(&offline, &AtomicBool::new(false))
        .await
        .unwrap_err();
    assert_eq!(error.error, Error::Configuration);
    let (base, server) = serve(vec![(200, "text/html", b"invalid".to_vec())]);
    let client = IrodoriClient::new(&base, Duration::from_secs(2)).unwrap();
    assert_eq!(
        client.catalog(&AtomicBool::new(false)).await.unwrap_err(),
        Error::InvalidResponse
    );
    server.join().unwrap();
}
#[tokio::test]
async fn busy_health_is_observable() {
    let (base, server) = serve(vec![json_response(json!({"status":"ready","busy":true}))]);
    let client = IrodoriClient::new(&base, Duration::from_secs(2)).unwrap();
    assert!(client.health(&AtomicBool::new(false)).await.unwrap().busy);
    server.join().unwrap();
}
