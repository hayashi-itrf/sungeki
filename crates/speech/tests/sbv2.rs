use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_speech::sbv2::{Error, Language, Sbv2Client, Settings};
fn settings() -> Settings {
    Settings {
        model_id: 2,
        speaker_id: 3,
        language: Language::JP,
        style: "嬉しい".into(),
        style_weight: 1.,
        length: 1.2,
        sdp_ratio: 0.2,
        noise: 0.6,
        noisew: 0.8,
        auto_split: true,
        split_interval: 0.5,
    }
}
fn server(status: &str, headers: &str, body: &[u8]) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let reply = format!("HTTP/1.1 {status}\r\n{headers}\r\nConnection: close\r\n\r\n");
    let body = body.to_vec();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        loop {
            let mut buf = [0; 1024];
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0);
            request.extend_from_slice(&buf[..n]);
            if request.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        stream.write_all(reply.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
        String::from_utf8(request).unwrap()
    });
    (address, handle)
}
#[tokio::test]
async fn explicit_settings_use_post_query_and_audio_remains_unverified() {
    let (url, handle) = server("200 OK", "Content-Length: 3", b"wav");
    let client = Sbv2Client::new(&url, Duration::from_secs(2)).unwrap();
    assert_eq!(
        client
            .synthesize("秘密&文", &settings(), &AtomicBool::new(false))
            .await
            .unwrap(),
        b"wav"
    );
    let request = handle.join().unwrap();
    assert!(request.starts_with("POST /voice?"));
    let path = request.split_whitespace().nth(1).unwrap();
    let parsed = reqwest::Url::parse(&format!("{url}{path}")).unwrap();
    let pairs: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
    assert_eq!(pairs["text"], "秘密&文");
    assert_eq!(pairs["model_id"], "2");
    assert_eq!(pairs["speaker_id"], "3");
    assert_eq!(pairs["style"], "嬉しい");
    assert_eq!(pairs["length"], "1.2");
    assert_eq!(pairs["language"], "JP");
    assert_eq!(pairs.len(), 12);
}
#[tokio::test]
async fn status_limits_and_catalog_are_checked_without_leaking_bodies() {
    for (status, headers, body, expected) in [
        (
            "500 Error",
            "Content-Length: 6",
            b"secret".as_slice(),
            Error::Http(500),
        ),
        (
            "302 Found",
            "Content-Length: 0\r\nLocation: http://127.0.0.1:1",
            b"".as_slice(),
            Error::Http(302),
        ),
        (
            "200 OK",
            "Content-Length: 5000000",
            b"".as_slice(),
            Error::TooLarge,
        ),
        (
            "200 OK",
            "Content-Length: 2",
            b"[]".as_slice(),
            Error::InvalidResponse,
        ),
    ] {
        let (url, handle) = server(status, headers, body);
        let error = Sbv2Client::new(&url, Duration::from_secs(2))
            .unwrap()
            .models(&AtomicBool::new(false))
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains("secret"));
        handle.join().unwrap();
    }
    let (url, handle) = server("200 OK", "Content-Length: 2", b"{}");
    assert!(
        Sbv2Client::new(&url, Duration::from_secs(2))
            .unwrap()
            .models(&AtomicBool::new(false))
            .await
            .unwrap()
            .is_object()
    );
    assert!(handle.join().unwrap().starts_with("GET /models/info "));
}
#[tokio::test]
async fn rejects_invalid_configuration_and_cancel_before_network() {
    for url in [
        "https://127.0.0.1",
        "http://example.com",
        "http://localhost",
        "http://user@127.0.0.1",
        "http://127.0.0.1/path",
        "http://127.0.0.1/?x=1",
    ] {
        assert!(matches!(
            Sbv2Client::new(url, Duration::from_secs(1)),
            Err(Error::Configuration)
        ));
    }
    let client = Sbv2Client::new("http://127.0.0.1:1", Duration::from_secs(1)).unwrap();
    assert_eq!(
        client
            .synthesize("本文", &settings(), &AtomicBool::new(true))
            .await
            .unwrap_err(),
        Error::Cancelled
    );
    for value in [f64::NAN, f64::INFINITY, 0., -1.] {
        let mut s = settings();
        s.length = value;
        assert!(s.validate().is_err());
    }
}

#[tokio::test]
async fn stalled_response_has_a_finite_timeout_without_text_in_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut bytes = [0; 4096];
        assert!(stream.read(&mut bytes).unwrap() > 0);
        std::thread::sleep(Duration::from_millis(150));
    });
    let error = Sbv2Client::new(&url, Duration::from_millis(50))
        .unwrap()
        .synthesize("private text", &settings(), &AtomicBool::new(false))
        .await
        .unwrap_err();
    assert_eq!(error, Error::Timeout);
    assert!(!error.to_string().contains("private"));
    handle.join().unwrap();
}
