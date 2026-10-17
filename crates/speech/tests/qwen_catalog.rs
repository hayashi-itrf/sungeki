use serde_json::{Value, json};
use sungeki_speech::qwen::{Catalog, Error, Settings};
fn settings() -> Settings {
    Settings {
        model: "qwen3-tts".into(),
        voice: "Vivian".into(),
        speed: 1.,
        language: "Japanese".into(),
        instruct: None,
    }
}
fn health() -> Value {
    json!({"status":"healthy","backend":{"name":"official","model_id":"Qwen/model-1", "ready":true},"version":"0.1.0", "device":{"vram_used":12}})
}
fn models() -> Value {
    json!({"data":[{"id":"qwen3-tts"},{"id":"tts-1"}]})
}
fn voices() -> Value {
    json!({"voices":[{"id":"Vivian","name":"Vivian","language":"English", "description":"Voice"}, {"id":"Ryan","name":"Ryan"}], "languages":["English","Japanese"]})
}
#[test]
fn order_and_transient_device_usage_do_not_change_selection() {
    let before = Catalog::parse(health(), models(), voices())
        .unwrap()
        .select(&settings())
        .unwrap();
    let mut h = health();
    h["device"]["vram_used"] = 999.into();
    let mut m = models();
    m["data"].as_array_mut().unwrap().reverse();
    let mut v = voices();
    v["voices"].as_array_mut().unwrap().reverse();
    v["languages"].as_array_mut().unwrap().reverse();
    assert_eq!(
        Catalog::parse(h, m, v)
            .unwrap()
            .select(&settings())
            .unwrap(),
        before
    );
    let mut h = health();
    h["backend"]["model_id"] = "Qwen/model-2".into();
    assert_ne!(
        Catalog::parse(h, models(), voices())
            .unwrap()
            .select(&settings())
            .unwrap(),
        before
    );
    let mut v = voices();
    v["voices"][0]["description"] = "New voice".into();
    assert_ne!(
        Catalog::parse(health(), models(), v)
            .unwrap()
            .select(&settings())
            .unwrap(),
        before
    );
}
#[test]
fn fallback_catalog_is_not_readiness_evidence() {
    for (status, ready) in [("initializing", false), ("healthy", false), ("error", true)] {
        let mut h = health();
        h["status"] = status.into();
        h["backend"]["ready"] = ready.into();
        assert_eq!(
            Catalog::parse(h, models(), voices()).unwrap_err(),
            Error::NotReady
        );
    }
}
#[test]
fn ambiguous_or_malformed_catalogs_are_rejected() {
    let mut m = models();
    m["data"][1] = m["data"][0].clone();
    assert_eq!(
        Catalog::parse(health(), m, voices()).unwrap_err(),
        Error::InvalidResponse
    );
    let mut v = voices();
    v["voices"][1] = v["voices"][0].clone();
    assert_eq!(
        Catalog::parse(health(), models(), v).unwrap_err(),
        Error::InvalidResponse
    );
    let mut v = voices();
    v["languages"] = json!(["Japanese", "Japanese"]);
    assert_eq!(
        Catalog::parse(health(), models(), v).unwrap_err(),
        Error::InvalidResponse
    );
    for key in ["model_id", "name"] {
        let mut h = health();
        h["backend"][key] = "".into();
        assert_eq!(
            Catalog::parse(h, models(), voices()).unwrap_err(),
            Error::InvalidResponse
        );
    }
}
#[test]
fn selection_must_exist_and_language_is_explicit() {
    let catalog = Catalog::parse(health(), models(), voices()).unwrap();
    for value in [
        Settings {
            model: "absent".into(),
            ..settings()
        },
        Settings {
            voice: "absent".into(),
            ..settings()
        },
        Settings {
            language: "absent".into(),
            ..settings()
        },
    ] {
        assert_eq!(catalog.select(&value), Err(Error::Configuration));
    }
    assert!(
        catalog
            .select(&Settings {
                language: "Auto".into(),
                ..settings()
            })
            .is_ok()
    );
}
#[tokio::test]
async fn reads_all_metadata_and_rejects_model_change_during_catalog_request() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::atomic::AtomicBool,
        time::Duration,
    };
    use sungeki_speech::qwen::QwenClient;
    for change in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut after = health();
            if change {
                after["backend"]["model_id"] = "changed".into();
            }
            for (path, body) in [
                ("/health", health()),
                ("/v1/models", models()),
                ("/v1/audio/voices", voices()),
                ("/health", after),
            ] {
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
                assert!(
                    String::from_utf8(request)
                        .unwrap()
                        .starts_with(&format!("GET {path} HTTP/1.1\r\n"))
                );
                let bytes = serde_json::to_vec(&body).unwrap();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    bytes.len()
                )
                .unwrap();
                stream.write_all(&bytes).unwrap();
            }
        });
        let result = QwenClient::new(&url, Duration::from_secs(2))
            .unwrap()
            .catalog(&AtomicBool::new(false))
            .await;
        if change {
            assert_eq!(result.unwrap_err(), Error::ModelChanged);
        } else {
            assert_eq!(
                result.unwrap().select(&settings()).unwrap().model_id,
                "Qwen/model-1"
            );
        }
        handle.join().unwrap();
    }
}
