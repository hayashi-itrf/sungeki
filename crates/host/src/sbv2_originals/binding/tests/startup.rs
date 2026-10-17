//! Exercises the managed path with a tiny Python HTTP child, never a TTS engine.
use super::*;
use crate::sbv2_startup::{self, Request};
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

pub(super) fn request(mode: &str) -> (Fixture, Request, std::path::PathBuf) {
    let (fixture, mut binding, catalog, settings, mut identity) = setup();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    binding.endpoint = format!("http://127.0.0.1:{port}");
    let pid = fixture.0.join("startup.pid");
    let mut body = serde_json::to_value(catalog.models()).unwrap();
    if mode == "changed_catalog" {
        body["0"]["device"] = json!("changed");
    }
    let body = if mode == "invalid" {
        "not json".into()
    } else {
        body.to_string()
    };
    let mut wav = crate::speech_output_tests::response().unverified_wav;
    if mode != "speech_wrong_rate" {
        wav[24..28].copy_from_slice(&44100u32.to_le_bytes());
        wav[28..32].copy_from_slice(&88200u32.to_le_bytes());
    }
    if mode == "speech_bad_wav" {
        wav = b"not a WAV".to_vec();
    }
    let wav_path = fixture.0.join("response.wav");
    std::fs::write(&wav_path, wav).unwrap();
    let action = match mode {
        "exit" => "raise SystemExit(7)".into(),
        "hang" => "time.sleep(60)".into(),
        "delayed" => "time.sleep(0.3)".into(),
        "changed_original" => format!(
            "Path({}).write_bytes(b'changed model')",
            json!(binding.originals.model)
        ),
        _ => "pass".into(),
    };
    // Replace only the fixture executable. The real engine, venv and models stay untouched.
    let script = format!(
        r##"#!/usr/bin/python3
import os, socket, time
from pathlib import Path
Path({pid}).write_text(str(os.getpid()))
{action}
s = socket.socket(fileno=0)
body = {body}.encode()
while True:
 c, _ = s.accept()
 request = c.recv(65536)
 payload = body
 if request.startswith(b'POST /voice?'):
  Path({voice_started}).write_bytes(request)
  if {mode} in ('speech_hang', 'speech_delayed'):
   time.sleep(60 if {mode} == 'speech_hang' else 0.3)
  if {mode} == 'speech_changed':
   Path({model}).write_bytes(b'changed during synthesis')
  payload = Path({wav_path}).read_bytes()
 c.sendall(b'HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: '+str(len(payload)).encode()+b'\r\nConnection: close\r\n\r\n'+payload)
 c.close()
"##,
        status = if mode == "retry" {
            "503 Service Unavailable"
        } else {
            "200 OK"
        },
        mode = json!(mode),
        model = json!(binding.originals.model),
        wav_path = json!(wav_path),
        voice_started = json!(fixture.0.join("voice.request")),
        pid = json!(pid),
        body = json!(body)
    );
    std::fs::write(&binding.engine.executable_target, script).unwrap();
    identity.engine_revision = binding.engine.revision(&AtomicBool::new(false)).unwrap();
    (
        fixture,
        Request {
            progress: Default::default(),
            binding,
            catalog,
            settings,
            identity,
            cpu_only: true,
        },
        pid,
    )
}
fn absent(path: &std::path::Path) {
    let pid: i32 = std::fs::read_to_string(path).unwrap().parse().unwrap();
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}
#[tokio::test]
async fn ready_requires_catalog_and_remeasured_originals_then_owner_stops() {
    let (_fixture, request, pid) = request("ready");
    let expected = request.identity.config_sha256.clone();
    let mut ready = sbv2_startup::start(
        request,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(3),
    )
    .await
    .unwrap_or_else(|failure| panic!("{}", failure.error));
    assert_eq!(ready.measured.config_sha256, expected);
    assert_eq!(ready.catalog.models().len(), 1);
    assert!(!ready.process.has_exited().unwrap());
    tokio::task::spawn_blocking(move || ready.process.stop(Duration::from_secs(2)).unwrap())
        .await
        .unwrap();
    absent(&pid);
}
#[tokio::test]
async fn failed_readiness_stops_and_reaps_its_process() {
    for mode in [
        "changed_catalog",
        "changed_original",
        "invalid",
        "exit",
        "hang",
        "retry",
    ] {
        let (_fixture, request, pid) = request(mode);
        let failure = match sbv2_startup::start(
            request,
            Arc::new(AtomicBool::new(false)),
            Duration::from_millis(700),
        )
        .await
        {
            Ok(_) => panic!("unexpected ready: {mode}"),
            Err(failure) => failure,
        };
        assert!(
            failure.cleanup_confirmed,
            "{mode}: {} cleanup={:?}",
            failure.error, failure.cleanup_error
        );
        assert!(failure.unsettled.is_none());
        assert!(failure.cleanup_error.is_none());
        if mode.starts_with("changed") {
            assert_eq!(failure.error.code, ErrorCode::ExternalModification);
        }
        absent(&pid);
    }
}
#[tokio::test]
async fn cancellation_during_readiness_reaps_child() {
    let (_fixture, request, pid) = request("hang");
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let path = pid.clone();
    let cancel_task = tokio::spawn(async move {
        for _ in 0..100 {
            if path.exists() {
                flag.store(true, Ordering::Release);
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("fixture child did not start");
    });
    let failure = match sbv2_startup::start(request, cancel, Duration::from_secs(3)).await {
        Ok(_) => panic!("unexpected ready"),
        Err(failure) => failure,
    };
    cancel_task.await.unwrap();
    assert_eq!(failure.error.code, ErrorCode::Cancelled);
    assert!(failure.cleanup_confirmed);
    absent(&pid);
}
#[tokio::test]
async fn occupied_endpoint_or_cancelled_request_does_not_spawn_or_adopt() {
    for pre_cancelled in [false, true] {
        let (_fixture, mut request, pid) = request("ready");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        request.binding.endpoint = format!("http://{address}");
        let failure = match sbv2_startup::start(
            request,
            Arc::new(AtomicBool::new(pre_cancelled)),
            Duration::from_secs(1),
        )
        .await
        {
            Ok(_) => panic!("adopted occupied endpoint"),
            Err(failure) => failure,
        };
        assert!(failure.cleanup_confirmed);
        assert!(!pid.exists());
        assert!(std::net::TcpStream::connect(address).is_ok());
        assert_eq!(
            failure.error.code,
            if pre_cancelled {
                ErrorCode::Cancelled
            } else {
                ErrorCode::IoError
            }
        );
    }
}
