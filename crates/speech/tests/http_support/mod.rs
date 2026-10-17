use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use sungeki_speech::voicevox::VoicevoxSettings;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

pub struct Reply {
    pub header: String,
    pub body: Vec<u8>,
    pub body_delay: Duration,
    pub cancel: Option<Arc<AtomicBool>>,
}
impl Reply {
    pub fn ok(body: Vec<u8>) -> Self {
        Self {
            header: format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            ),
            body,
            body_delay: Duration::ZERO,
            cancel: None,
        }
    }
}
#[derive(Debug)]
pub struct Request {
    pub url: reqwest::Url,
    pub body: Vec<u8>,
}
pub async fn server(replies: Vec<Reply>) -> (String, JoinHandle<Vec<Request>>) {
    server_with_method(replies, "POST").await
}
pub async fn server_with_method(
    replies: Vec<Reply>,
    method: &'static str,
) -> (String, JoinHandle<Vec<Request>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let mut requests = vec![];
        for reply in replies {
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut bytes = vec![];
            let split = loop {
                let mut chunk = [0; 4096];
                let count = stream.read(&mut chunk).await.unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&chunk[..count]);
                if let Some(split) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break split + 4;
                }
            };
            let header = String::from_utf8(bytes[..split].to_vec()).unwrap();
            let first = header.lines().next().unwrap();
            assert_eq!(first.split_whitespace().next().unwrap(), method, "{first}");
            let path = first.split_whitespace().nth(1).unwrap();
            let length: usize = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .unwrap_or(0);
            while bytes.len() - split < length {
                let mut chunk = [0; 4096];
                let count = stream.read(&mut chunk).await.unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&chunk[..count]);
            }
            requests.push(Request {
                url: reqwest::Url::parse(&format!("http://localhost{path}")).unwrap(),
                body: bytes[split..].to_vec(),
            });
            if let Some(cancel) = reply.cancel {
                cancel.store(true, Ordering::Release);
            }
            let _ = stream.write_all(reply.header.as_bytes()).await;
            tokio::time::sleep(reply.body_delay).await;
            let _ = stream.write_all(&reply.body).await;
        }
        requests
    });
    (base, handle)
}
pub fn query() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "accent_phrases": [{"moras":[{"text":"ア","vowel":"a","vowel_length":0.1,"pitch":5.5}], "accent":1}],
        "speedScale":1.0,"pitchScale":0.0,"intonationScale":1.0,"volumeScale":1.0,
        "prePhonemeLength":0.1,"postPhonemeLength":0.1,"outputSamplingRate":24000,"outputStereo":false
    })).unwrap()
}
pub fn settings() -> VoicevoxSettings {
    VoicevoxSettings {
        style_id: 42,
        speed_scale: 1.2,
        pitch_scale: 0.0,
        intonation_scale: 1.0,
        volume_scale: 0.7,
        pre_phoneme_length: 0.2,
        post_phoneme_length: 0.3,
        pause_length: None,
        pause_length_scale: 1.0,
        output_sampling_rate: 48000,
        output_stereo: false,
        enable_interrogative_upspeak: false,
    }
}
