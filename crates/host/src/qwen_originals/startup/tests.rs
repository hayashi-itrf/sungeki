use super::*;
use crate::qwen_originals::{Paths, inspect};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
};
pub(super) struct Fixture(pub(super) PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub(super) fn request(mode: &str) -> (Fixture, Request, PathBuf) {
    let root = std::env::temp_dir().join(format!("qwen-startup-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    for name in ["engine/api", "model", "voice", "work", "venv/bin"] {
        fs::create_dir_all(root.join(name)).unwrap();
    }
    for (name, value) in [
        ("engine/api/main.py", "main"),
        ("model/config.json", "{}"),
        ("voice/mapping", "mapping"),
        ("launch.py", "launcher"),
        ("venv/pyvenv.cfg", "include-system-site-packages = false\n"),
    ] {
        fs::write(root.join(name), value).unwrap();
    }
    let model = root.join("model").to_str().unwrap().to_owned();
    let health = json!({"status":"healthy","backend":{"name":"official","model_id":model,"ready":true},"version":"test"});
    let models = json!({"data":[{"id":"qwen3-tts"}]});
    let voices = json!({"voices":[{"id":"Vivian","name":"Vivian"}],"languages":["Japanese"]});
    let settings = Settings {
        model: "qwen3-tts".into(),
        voice: "Vivian".into(),
        language: "Japanese".into(),
        speed: 1.,
        instruct: None,
    };
    let selection = Catalog::parse(health.clone(), models.clone(), voices.clone())
        .unwrap()
        .select(&settings)
        .unwrap();
    let pid = root.join("pid");
    let mut actual = health;
    if mode == "changed" {
        actual["backend"]["model_id"] = "changed".into();
    }
    if mode == "not_ready" {
        actual["status"] = "initializing".into();
        actual["backend"]["ready"] = false.into();
    }
    let script = format!(
        r##"#!/usr/bin/python3
import socket, os, time, json, struct
from pathlib import Path
Path({pid}).write_text(str(os.getpid()))
mode={mode}
if mode=='exit': raise SystemExit(7)
if mode=='hang': time.sleep(60)
if mode=='slow_start': time.sleep(2)
if mode=='changed_original': Path({config}).write_text('changed')
s=socket.socket(fileno=0)
bodies={{'/health':{health},'/v1/models':{models},'/v1/audio/voices':{voices}}}
while True:
 c,_=s.accept()
 req=c.recv(65536)
 while b'\r\n\r\n' not in req: req+=c.recv(65536)
 header, body_so_far=req.split(b'\r\n\r\n',1)
 length=0
 for line in header.split(b'\r\n')[1:]:
  key, _, value=line.partition(b':')
  if key.lower()==b'content-length': length=int(value)
 while len(body_so_far)<length: body_so_far+=c.recv(65536)
 path=req.split(b' ')[1].decode()
 body=bodies.get(path,'invalid').encode()
 if path=='/v1/audio/speech':
  Path({speech_started}).write_text('started')
  if mode=='speech_hang': time.sleep(60)
  if mode=='speech_changed': Path({config}).write_text('changed')
  rate=48000 if mode=='speech_wrong_rate' else 24000
  pcm=b'\x01\x00'*240
  body=b'RIFF'+struct.pack('<I',36+len(pcm))+b'WAVEfmt '+struct.pack('<IHHIIHH',16,1,1,rate,rate*2,2,16)+b'data'+struct.pack('<I',len(pcm))+pcm
  if mode=='speech_bad': body=b'bad wav' 
 try: c.sendall(b'HTTP/1.1 200 OK\r\nContent-Length: '+str(len(body)).encode()+b'\r\nConnection: close\r\n\r\n'+body)
 except BrokenPipeError: pass
 c.close()
"##,
        pid = json!(pid),
        speech_started = json!(root.join("speech.started")),
        mode = json!(mode),
        config = json!(root.join("model/config.json")),
        health = json!(actual.to_string()),
        models = json!(models.to_string()),
        voices = json!(voices.to_string())
    );
    fs::write(root.join("python-real"), script).unwrap();
    fs::set_permissions(root.join("python-real"), fs::Permissions::from_mode(0o700)).unwrap();
    symlink(root.join("python-real"), root.join("venv/bin/python")).unwrap();
    let originals = Paths {
        model: BTreeMap::from([("config.json".into(), root.join("model/config.json"))]),
        voice: BTreeMap::from([("mapping".into(), root.join("voice/mapping"))]),
        engine: [
            ("api/main.py", "engine/api/main.py"),
            ("next_launcher.py", "launch.py"),
            ("python_executable", "python-real"),
            ("pyvenv.cfg", "venv/pyvenv.cfg"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), root.join(v)))
        .collect(),
    };
    let measured = inspect(&originals, &AtomicBool::new(false)).unwrap();
    let identity = RuntimeIdentity {
        backend: "official".into(),
        model_id: model,
        model_manifest_sha256: measured.model_manifest_sha256,
        engine_manifest_sha256: measured.engine_manifest_sha256,
        voice_manifest_sha256: measured.voice_manifest_sha256,
        sample_rate: 24000,
        channels: 1,
    };
    let binding = Binding {
        engine_root: root.join("engine"),
        model_root: root.join("model"),
        voice_root: root.join("voice"),
        work_root: root.join("work"),
        executable: root.join("venv/bin/python"),
        executable_target: root.join("python-real"),
        launcher: root.join("launch.py"),
        originals,
    };
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    (
        Fixture(root),
        Request {
            progress: Default::default(),
            binding,
            port,
            settings,
            selection,
            identity,
        },
        pid,
    )
}
pub(super) fn absent(pid: &std::path::Path) {
    let id: i32 = fs::read_to_string(pid).unwrap().parse().unwrap();
    assert_eq!(unsafe { libc::kill(id, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}
#[tokio::test]
async fn owns_ready_child_and_reaps_on_stop() {
    let (_fixture, request, pid) = request("ready");
    let mut ready = start(
        request,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(3),
    )
    .await
    .unwrap_or_else(|f| panic!("{}", f.error));
    assert!(!ready.process.has_exited().unwrap());
    tokio::task::spawn_blocking(move || ready.process.stop(Duration::from_secs(2)).unwrap())
        .await
        .unwrap();
    absent(&pid);
}
#[tokio::test]
async fn failure_and_cancellation_reap_owned_child() {
    for mode in [
        "changed",
        "changed_original",
        "not_ready",
        "hang",
        "exit",
        "cancel",
    ] {
        let (_fixture, request, pid) = request(if mode == "cancel" { "hang" } else { mode });
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let task = tokio::spawn(async move { start(request, flag, Duration::from_secs(5)).await });
        if mode == "cancel" {
            // Cancellation must exercise a launched child, not race preflight.
            tokio::time::timeout(Duration::from_secs(4), async {
                while !pid.exists() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("fixture child did not start");
            cancel.store(true, Ordering::Release);
        }
        let failure = match task.await.unwrap() {
            Ok(_) => panic!("unexpected ready {mode}"),
            Err(f) => f,
        };
        assert!(failure.cleanup_confirmed, "{}", failure.error);
        assert!(failure.unsettled.is_none());
        assert!(failure.cleanup_error.is_none());
        absent(&pid);
    }
}
#[tokio::test]
async fn occupied_port_is_not_adopted_or_stopped() {
    let (_fixture, mut request, pid) = request("ready");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    request.port = listener.local_addr().unwrap().port();
    assert!(
        start(
            request,
            Arc::new(AtomicBool::new(false)),
            Duration::from_secs(1)
        )
        .await
        .is_err()
    );
    assert!(!pid.exists());
    assert!(listener.local_addr().is_ok());
}

mod exchange;

mod job;

mod configuration;

mod api;

mod build;
