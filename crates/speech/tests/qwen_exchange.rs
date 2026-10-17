use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_media::ExpectedAudio;
use sungeki_project::PronunciationEntry;
use sungeki_speech::qwen::{
    Catalog, Error, ExchangeStage, PreparedQwenInput, QwenClient, Settings,
};
fn health(model: &str) -> Value {
    json!({"status":"healthy","backend":{"name":"official","model_id":model,"ready":true},"version":"0.1"})
}
fn models() -> Value {
    json!({"data":[{"id":"qwen3-tts"}]})
}
fn voices() -> Value {
    json!({"voices":[{"id":"Vivian","name":"Vivian"}],"languages":["Japanese"]})
}
fn catalog() -> Catalog {
    Catalog::parse(health("model-a"), models(), voices()).unwrap()
}
fn settings() -> Settings {
    Settings {
        model: "qwen3-tts".into(),
        voice: "Vivian".into(),
        language: "Japanese".into(),
        speed: 1.,
        instruct: None,
    }
}
fn expected() -> ExpectedAudio {
    ExpectedAudio {
        sample_rate: 24000,
        channels: 1,
    }
}
fn input() -> PreparedQwenInput {
    PreparedQwenInput::new(
        "Codex",
        None,
        &[PronunciationEntry {
            written: "Codex".into(),
            spoken: "コーデックス".into(),
        }],
        &settings(),
        &catalog(),
        expected(),
    )
    .unwrap()
    .bind_identity(&identity())
    .unwrap()
}
#[test]
fn reading_snapshot_limits_and_output_validation() {
    let input = input();
    assert_eq!(input.subtitle(), "Codex");
    assert_eq!(input.resolved().text, "コーデックス");
    let changed = PreparedQwenInput::new(
        "Codex",
        Some("別の読み"),
        input.dictionary(),
        input.settings(),
        &catalog(),
        expected(),
    )
    .unwrap();
    assert_ne!(input.fingerprint(), changed.fingerprint());
    let dictionary = [PronunciationEntry {
        written: "X".into(),
        spoken: "あ".repeat(300),
    }];
    // Dictionary expansion must be checked before dispatch, not just source length.
    assert!(
        PreparedQwenInput::new(
            &"X".repeat(15),
            None,
            &dictionary,
            &settings(),
            &catalog(),
            expected()
        )
        .is_err()
    );
    assert!(input.verify_audio(b"not-wav".to_vec()).is_err());
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut wav = hound::WavWriter::new(
            &mut bytes,
            hound::WavSpec {
                channels: 1,
                sample_rate: 24000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..240 {
            wav.write_sample(123i16).unwrap();
        }
        wav.finalize().unwrap();
    }
    let bytes = bytes.into_inner();
    assert_eq!(
        input
            .verify_audio(bytes.clone())
            .unwrap()
            .info()
            .duration_us,
        10000
    );
    let mismatch = PreparedQwenInput::new(
        "Codex",
        None,
        &[],
        &settings(),
        &catalog(),
        ExpectedAudio {
            sample_rate: 48000,
            channels: 1,
        },
    )
    .unwrap();
    assert!(mismatch.verify_audio(bytes).is_err());
}
fn server(mode: u8) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut replies = Vec::new();
        let model = if mode == 1 { "changed" } else { "model-a" };
        for (path, value) in [
            ("/health", health(model)),
            ("/v1/models", models()),
            ("/v1/audio/voices", voices()),
            ("/health", health(model)),
        ] {
            replies.push((path, 200, serde_json::to_vec(&value).unwrap()));
        }
        if mode != 1 {
            replies.push((
                "/v1/audio/speech",
                if mode == 2 { 500 } else { 200 },
                b"raw".to_vec(),
            ));
            if mode != 2 {
                let model = if mode == 3 { "changed" } else { "model-a" };
                for (path, value) in [
                    ("/health", health(model)),
                    ("/v1/models", models()),
                    ("/v1/audio/voices", voices()),
                    ("/health", health(model)),
                ] {
                    replies.push((path, 200, serde_json::to_vec(&value).unwrap()));
                }
            }
        }
        for (path, status, body) in replies {
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
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&request[..end]);
                    let length: usize = head
                        .lines()
                        .find_map(|l| {
                            let (k, v) = l.split_once(':')?;
                            k.eq_ignore_ascii_case("content-length")
                                .then(|| v.trim().parse().unwrap())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let request = String::from_utf8(request).unwrap();
            let method = if path == "/v1/audio/speech" {
                "POST"
            } else {
                "GET"
            };
            assert!(request.starts_with(&format!("{method} {path} HTTP/1.1\r\n")));
            if method == "POST" {
                let body: Value =
                    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                assert_eq!(body["input"], "コーデックス");
            }
            write!(
                stream,
                "HTTP/1.1 {status} Result\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    (url, handle)
}
#[tokio::test]
async fn exchange_rejects_changed_selection_and_preserves_settlement_stage() {
    for mode in 0..4 {
        let (url, handle) = server(mode);
        let client = QwenClient::new(&url, Duration::from_secs(2)).unwrap();
        let result = client
            .exchange_prepared(&input(), &AtomicBool::new(false))
            .await;
        if mode == 0 {
            assert_eq!(result.unwrap(), b"raw");
        } else {
            let failure = result.unwrap_err();
            assert_eq!(
                failure.stage,
                match mode {
                    1 => ExchangeStage::BeforeSynthesis,
                    2 => ExchangeStage::Synthesis,
                    _ => ExchangeStage::AfterSynthesis,
                }
            );
            assert_eq!(failure.requires_engine_settlement(), mode == 2);
            assert_eq!(
                failure.error,
                if mode == 2 {
                    Error::Http(500)
                } else {
                    Error::ModelChanged
                }
            );
        }
        handle.join().unwrap();
    }
}
#[tokio::test]
async fn cancelled_preflight_does_not_dispatch() {
    let client = QwenClient::new("http://127.0.0.1:1", Duration::from_secs(1)).unwrap();
    let failure = client
        .exchange_prepared(&input(), &AtomicBool::new(true))
        .await
        .unwrap_err();
    assert_eq!(failure.stage, ExchangeStage::BeforeSynthesis);
    assert_eq!(failure.error, Error::Cancelled);
}

fn identity() -> sungeki_speech::qwen::RuntimeIdentity {
    sungeki_speech::qwen::RuntimeIdentity {
        backend: "official".into(),
        model_id: "model-a".into(),
        model_manifest_sha256: "a".repeat(64),
        engine_manifest_sha256: "b".repeat(64),
        voice_manifest_sha256: "c".repeat(64),
        sample_rate: 24000,
        channels: 1,
    }
}
#[tokio::test]
async fn unidentified_input_is_rejected_before_network() {
    let input =
        PreparedQwenInput::new("hello", None, &[], &settings(), &catalog(), expected()).unwrap();
    let client = QwenClient::new("http://127.0.0.1:1", Duration::from_secs(1)).unwrap();
    let failure = client
        .exchange_prepared(&input, &AtomicBool::new(false))
        .await
        .unwrap_err();
    assert_eq!(failure.stage, ExchangeStage::BeforeSynthesis);
    assert_eq!(failure.error, Error::Configuration);
    let bound = input.clone().bind_identity(&identity()).unwrap();
    assert_ne!(input.fingerprint(), bound.fingerprint());
    assert!(bound.bind_identity(&identity()).is_err());
    let mut changed = identity();
    changed.model_id = "other".into();
    assert!(input.clone().bind_identity(&changed).is_err());
    changed = identity();
    changed.sample_rate = 48000;
    assert!(input.clone().bind_identity(&changed).is_err());
    changed = identity();
    changed.voice_manifest_sha256 = "C".repeat(64);
    assert!(input.bind_identity(&changed).is_err());
}
