use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_speech::qwen::{Error, QwenClient, Settings, canonical_endpoint};
fn settings() -> Settings {
    Settings {
        model: "qwen3-tts".into(),
        voice: "Vivian".into(),
        speed: 1.,
        language: "Japanese".into(),
        instruct: Some("落ち着いて".into()),
    }
}
fn server(status: &str, headers: &str, body: &[u8]) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let reply = format!("HTTP/1.1 {status}\r\n{headers}\r\nConnection: close\r\n\r\n");
    let body = body.to_vec();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        loop {
            let mut buf = [0; 2048];
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0);
            request.extend_from_slice(&buf[..n]);
            if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&request[..end]);
                let length: usize = head
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().unwrap())
                    })
                    .unwrap();
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        stream.write_all(reply.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
        String::from_utf8(request).unwrap()
    });
    (url, handle)
}
#[tokio::test]
async fn unicode_limit_and_explicit_json_settings() {
    let (url, server) = server("200 OK", "Content-Length: 3", b"raw");
    let text = "あ".repeat(4096);
    let result = QwenClient::new(&url, Duration::from_secs(2))
        .unwrap()
        .synthesize(&text, &settings(), &AtomicBool::new(false))
        .await
        .unwrap();
    assert_eq!(result, b"raw"); // Transport does not certify WAV/model identity.
    let request = server.join().unwrap();
    assert!(request.starts_with("POST /v1/audio/speech HTTP/1.1\r\n"));
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(
        body,
        serde_json::json!({"model":"qwen3-tts", "voice":"Vivian", "speed":1., "language":"Japanese", "instruct":"落ち着いて", "input":text, "response_format":"wav", "stream":false, "normalization_options":{"normalize":false}})
    );
}
#[tokio::test]
async fn rejects_before_contacting_engine() {
    let client = QwenClient::new("http://127.0.0.1:1", Duration::from_secs(1)).unwrap();
    for text in ["".into(), " \n".into(), "a\0b".into(), "あ".repeat(4097)] {
        assert_eq!(
            client
                .synthesize(&text, &settings(), &AtomicBool::new(false))
                .await,
            Err(Error::Configuration)
        );
    }
    assert_eq!(
        client
            .synthesize("hello", &settings(), &AtomicBool::new(true))
            .await,
        Err(Error::Cancelled)
    );
    for speed in [f64::NAN, f64::INFINITY, 0.24, 4.01] {
        let mut value = settings();
        value.speed = speed;
        assert_eq!(
            client
                .synthesize("hello", &value, &AtomicBool::new(false))
                .await,
            Err(Error::Configuration)
        );
    }
}
#[tokio::test]
async fn rejects_redirect_errors_and_declared_oversize() {
    for (status, headers, body, expected) in [
        (
            "500 Error",
            "Content-Length: 6".into(),
            b"secret".as_slice(),
            Error::Http(500),
        ),
        (
            "302 Found",
            "Content-Length: 0\r\nLocation: http://127.0.0.1:1".into(),
            b"".as_slice(),
            Error::Http(302),
        ),
        (
            "200 OK",
            format!("Content-Length: {}", sungeki_media::MAX_WAV_BYTES + 1),
            b"".as_slice(),
            Error::TooLarge,
        ),
    ] {
        let (url, handle) = server(status, &headers, body);
        assert_eq!(
            QwenClient::new(&url, Duration::from_secs(2))
                .unwrap()
                .synthesize("hello", &settings(), &AtomicBool::new(false))
                .await,
            Err(expected)
        );
        handle.join().unwrap();
    }
}
#[test]
fn endpoint_and_timeout_restrictions() {
    for url in [
        "https://127.0.0.1",
        "http://example.com",
        "http://localhost",
        "http://192.168.1.2",
        "http://user@127.0.0.1",
        "http://127.0.0.1/path",
        "http://127.0.0.1?q=1",
        "http://127.0.0.1/#x",
    ] {
        assert_eq!(canonical_endpoint(url), Err(Error::Configuration));
    }
    assert!(canonical_endpoint("http://[::1]:8880").is_ok());
    assert!(QwenClient::new("http://127.0.0.1", Duration::ZERO).is_err());
    assert!(QwenClient::new("http://127.0.0.1", Duration::from_secs(301)).is_err());
}
