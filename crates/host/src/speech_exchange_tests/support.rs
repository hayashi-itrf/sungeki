use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
pub const ENGINE: &str = "c7b58856-bd56-4aa1-afb7-b8415f824b06";
pub struct Reply {
    pub on_request: Option<Box<dyn FnOnce() + Send>>,
    pub path: &'static str,
    pub body: Vec<u8>,
    pub delay: Duration,
    pub cancel: Option<Arc<AtomicBool>>,
}
impl Reply {
    pub fn new(path: &'static str, body: impl Into<String>) -> Self {
        Self {
            on_request: None,
            path,
            body: body.into().into_bytes(),
            delay: Duration::ZERO,
            cancel: None,
        }
    }
}
pub fn metadata(style: u32) -> Vec<Reply> {
    let engine = || {
        vec![
            Reply::new(
                "/engine_manifest",
                format!(r#"{{"uuid":"{ENGINE}","name":"test"}}"#),
            ),
            Reply::new("/version", "\"1\""),
            Reply::new("/core_versions", "[\"1\"]"),
        ]
    };
    let mut replies = engine();
    replies.extend(engine());
    replies.push(Reply::new("/speakers", format!(r#"[{{"name":"test","speaker_uuid":"{ENGINE}","version":"1","styles":[{{"name":"normal","id":{style},"type":"talk"}}]}}]"#)));
    replies.extend(engine());
    replies
}
pub fn analysis() -> Reply {
    Reply::new(
        "/audio_query",
        r#"{"accent_phrases":[],"speedScale":1,"pitchScale":0,"intonationScale":1,"volumeScale":1,"prePhonemeLength":0.1,"postPhonemeLength":0.1,"outputSamplingRate":24000,"outputStereo":false}"#,
    )
}
pub async fn server(replies: Vec<Reply>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let mut requests = vec![];
        for reply in replies {
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut bytes = vec![];
            let split = loop {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(i) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let header = String::from_utf8(bytes[..split].to_vec()).unwrap();
            let line = header.lines().next().unwrap().to_string();
            assert!(line.contains(reply.path), "{line}");
            assert!(line.starts_with(
                if reply.path == "/audio_query" || reply.path == "/synthesis" {
                    "POST "
                } else {
                    "GET "
                }
            ));
            let len: usize = header
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse().unwrap())
                })
                .unwrap_or(0);
            while bytes.len() - split < len {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
            }
            requests.push(line);
            if let Some(action) = reply.on_request {
                action();
            }
            if let Some(cancel) = reply.cancel {
                cancel.store(true, Ordering::Release);
            }
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                reply.body.len()
            );
            let _ = stream.write_all(header.as_bytes()).await;
            tokio::time::sleep(reply.delay).await;
            let _ = stream.write_all(&reply.body).await;
        }
        requests
    });
    (base, handle)
}
