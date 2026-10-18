use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::Duration,
};
use sungeki_project::{LocalSpeechEngine, LocalVoiceSettings};
fn server(
    responses: Vec<serde_json::Value>,
) -> (String, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    let handle = std::thread::spawn(move || {
        for response in responses {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            log.lock()
                .unwrap()
                .push(String::from_utf8(request).unwrap());
            let body = serde_json::to_vec(&response).unwrap();
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            socket.write_all(&body).unwrap();
        }
    });
    (address, requests, handle)
}
#[tokio::test]
async fn aivis_preserves_negative_style_ids_and_catalog_changes_identity() {
    let speaker = serde_json::json!([{"speaker_uuid":"5680ac39-43c9-487a-bc3e-018c0d29cc38","name":"test","version":"1","styles":[{"id":-12,"name":"normal"}]}]);
    let mut next = speaker.clone();
    next[0]["version"] = serde_json::json!("2");
    let (url, log, worker) = server(vec![
        serde_json::json!({"name":"AivisSpeech Engine"}),
        serde_json::json!("1.2.0"),
        speaker,
        serde_json::json!({"name":"AivisSpeech Engine"}),
        serde_json::json!("1.2.0"),
        next,
    ]);
    let client =
        LocalSpeechClient::new(&url, LocalSpeechEngine::AivisSpeech, Duration::from_secs(3))
            .unwrap();
    let a = client.catalog(&AtomicBool::new(false)).await.unwrap();
    let b = client.catalog(&AtomicBool::new(false)).await.unwrap();
    assert_eq!(a.speakers[0].styles[0].id, -12);
    assert_ne!(a.version, b.version);
    assert!(log.lock().unwrap().iter().all(|r| r.starts_with("GET ")));
    worker.join().unwrap();
}
#[tokio::test]
async fn coeiroink_reads_v2_fields_and_does_not_use_voicevox_routes() {
    let (url, log, worker) = server(vec![
        serde_json::json!({"device":"cpu","version":"2.13.0"}),
        serde_json::json!([{"speakerUuid":"3c37646f-3881-5374-2a83-149267990abc","speakerName":"test","styles":[{"styleId":0,"styleName":"calm"}]}]),
    ]);
    let client =
        LocalSpeechClient::new(&url, LocalSpeechEngine::Coeiroink, Duration::from_secs(3)).unwrap();
    let c = client.catalog(&AtomicBool::new(false)).await.unwrap();
    assert_eq!(c.speakers[0].styles[0].name, "calm");
    assert!(
        log.lock()
            .unwrap()
            .iter()
            .all(|r| r.starts_with("GET /v1/"))
    );
    worker.join().unwrap();
}
#[test]
fn local_settings_reject_invalid_numbers_but_allow_signed_aivis_styles() {
    let mut settings = LocalVoiceSettings::AivisSpeech {
        speaker_uuid: "5680ac39-43c9-487a-bc3e-018c0d29cc38".into(),
        style_id: -1,
        speed_scale: 1.,
        pitch_scale: 0.,
        emotion_scale: 1.,
        tempo_dynamics_scale: 1.,
        volume_scale: 1.,
        pre_phoneme_length: 0.1,
        post_phoneme_length: 0.1,
    };
    assert!(settings.validate().is_ok());
    if let LocalVoiceSettings::AivisSpeech { emotion_scale, .. } = &mut settings {
        *emotion_scale = f64::NAN;
    }
    assert!(settings.validate().is_err());
    for url in [
        "https://127.0.0.1:10101/",
        "http://example.com/",
        "http://127.0.0.1/a",
        "http://user@127.0.0.1/",
    ] {
        assert!(
            LocalSpeechClient::new(url, LocalSpeechEngine::AivisSpeech, Duration::from_secs(1))
                .is_err()
        );
    }
}
#[tokio::test]
async fn aivis_policy_requests_urls_instead_of_large_embedded_portraits() {
    let (url, log, worker) = server(vec![
        serde_json::json!({"policy":"voice policy","portrait":"http://127.0.0.1/resource","style_infos":[{"voice_samples":["a".repeat(9*1024*1024)]}]}),
    ]);
    let client =
        LocalSpeechClient::new(&url, LocalSpeechEngine::AivisSpeech, Duration::from_secs(3))
            .unwrap();
    assert_eq!(
        client
            .policy(
                "5680ac39-43c9-487a-bc3e-018c0d29cc38",
                &AtomicBool::new(false)
            )
            .await
            .unwrap(),
        "voice policy"
    );
    assert!(log.lock().unwrap()[0].contains("resource_format=url"));
    worker.join().unwrap();
}
