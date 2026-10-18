use super::*;
use crate::irodori_originals::Paths;
use serde_json::json;
use std::path::PathBuf;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
use sungeki_speech::irodori::{ModelCapabilities, ReferenceKind, RegisteredVoice};
fn fixture(mode: &str) -> (impl Drop, Request, PathBuf) {
    let root = std::env::temp_dir().join(format!("irodori-bind-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    struct Clean(PathBuf);
    impl Drop for Clean {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _clean = Clean(root.clone());
    for name in [
        "engine/irodori_tts",
        "model",
        "codec",
        "voice",
        "worker",
        "work",
        "venv/bin",
    ] {
        fs::create_dir_all(root.join(name)).unwrap();
    }
    for name in [
        "engine/irodori_tts/inference_runtime.py",
        "model/checkpoint",
        "codec/codec",
        "voice/registry",
        "python-real",
    ] {
        fs::write(root.join(name), "fixture").unwrap();
    }
    fs::write(
        root.join("venv/pyvenv.cfg"),
        "include-system-site-packages = false\n",
    )
    .unwrap();
    fs::set_permissions(root.join("python-real"), fs::Permissions::from_mode(0o700)).unwrap();
    symlink(root.join("python-real"), root.join("venv/bin/python")).unwrap();
    let mut engine: BTreeMap<String, PathBuf> = [
        ("python_executable", "python-real"),
        ("pyvenv.cfg", "venv/pyvenv.cfg"),
        (
            "irodori_tts/inference_runtime.py",
            "engine/irodori_tts/inference_runtime.py",
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), root.join(v)))
    .collect();
    for name in [
        "launch.py",
        "environment.py",
        "bridge.py",
        "audio.py",
        "server.py",
    ] {
        let path = root.join("worker").join(name);
        fs::write(&path, "fixture").unwrap();
        engine.insert(format!("next/{name}"), path);
    }
    let b = Binding {
        engine_root: root.join("engine"),
        checkpoint: root.join("model/checkpoint"),
        codec: root.join("codec/codec"),
        registry: root.join("voice/registry"),
        work_root: root.join("work"),
        dependency_cache: None,
        executable: root.join("venv/bin/python"),
        executable_target: root.join("python-real"),
        launcher: root.join("worker/launch.py"),
        device: "cpu".into(),
        precision: "fp32".into(),
        originals: Paths {
            model: BTreeMap::from([("checkpoint".into(), root.join("model/checkpoint"))]),
            codec: BTreeMap::from([("codec".into(), root.join("codec/codec"))]),
            voice: BTreeMap::from([("registry".into(), root.join("voice/registry"))]),
            engine,
        },
    };
    fs::write(
        &b.registry,
        r#"[{"id":"none","name":"参照なし","reference":"none","caption":null}]"#,
    )
    .unwrap();
    let model = ModelCapabilities {
        checkpoint: "model".into(),
        speaker_conditioning: true,
        caption_conditioning: false,
        sample_rate: 48000,
        channels: 1,
    };
    let voices = vec![RegisteredVoice {
        id: "none".into(),
        name: "参照なし".into(),
        reference: ReferenceKind::None,
        caption: None,
    }];
    let mut actual = serde_json::to_value(&model).unwrap();
    if mode == "changed" {
        actual["checkpoint"] = "wrong-model".into();
    }
    let catalog = json!({"model":actual,"voices":voices});
    let pid = root.join("pid");
    let script = format!(
        r##"#!/usr/bin/python3
import json, os, socket, time, struct
from pathlib import Path
Path({pid}).write_text(str(os.getpid()))
mode={mode}
if mode=='exit': raise SystemExit(7)
if mode=='hang': time.sleep(60)
if mode=='changed_file': Path({original}).write_text('modified')
s=socket.socket(fileno=0)
while True:
 c,_=s.accept()
 req=c.recv(65536)
 while b'\r\n\r\n' not in req: req+=c.recv(65536)
 path=req.split(b' ')[1].decode()
 body=({health} if path=='/v1/health' else {catalog}).encode()
 kind=b'application/json'
 if path=='/v1/synthesize':
  Path('speech.started').touch()
  if mode=='speech_hang': time.sleep(60)
  if mode=='speech_changed': Path({original}).write_text('modified')
  rate=24000 if mode=='speech_wrong_rate' else 48000
  pcm=b'\x01\x00'*480
  body=b'RIFF'+struct.pack('<I',36+len(pcm))+b'WAVEfmt '+struct.pack('<IHHIIHH',16,1,1,rate,rate*2,2,16)+b'data'+struct.pack('<I',len(pcm))+pcm
  if mode=='speech_bad': body=b'bad waveform'
  kind=b'audio/wav'
 try: c.sendall(b'HTTP/1.1 200 OK\r\nContent-Type: '+kind+b'\r\nContent-Length: '+str(len(body)).encode()+b'\r\nConnection: close\r\n\r\n'+body)
 except BrokenPipeError: pass
 c.close()
"##,
        pid = json!(pid),
        mode = json!(mode),
        original = json!(b.codec),
        health = json!(json!({"status":"ready","busy":mode=="busy"}).to_string()),
        catalog = json!(catalog.to_string())
    );
    fs::write(&b.executable_target, script).unwrap();
    let (_, m) = b
        .measured_command("model", &AtomicBool::new(false))
        .unwrap();
    let identity = RuntimeIdentity {
        checkpoint: "model".into(),
        model_manifest_sha256: m.model_manifest_sha256,
        codec_manifest_sha256: m.codec_manifest_sha256,
        engine_manifest_sha256: m.engine_manifest_sha256,
        voice_manifest_sha256: m.voice_manifest_sha256,
        sample_rate: 48000,
        channels: 1,
    };
    let settings = Settings {
        voice_id: "none".into(),
        caption: None,
        cfg_scale: 3.,
        num_steps: 40,
        seed: 42,
    };
    let selection = Catalog::new(model, voices)
        .unwrap()
        .select(&settings, &identity)
        .unwrap()
        .audio_source()
        .clone();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    (
        _clean,
        Request {
            progress: Default::default(),
            binding: b,
            selection,
            port,
            settings,
            identity,
        },
        pid,
    )
}
fn gone(pid: &std::path::Path) {
    if let Ok(text) = fs::read_to_string(pid) {
        let pid = text.parse::<i32>().unwrap();
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            -1,
            "child must have been reaped"
        );
    }
}
#[tokio::test]
async fn managed_start_rechecks_files_and_owns_stop() {
    let (_f, request, pid) = fixture("ready");
    let mut ready = start(
        request,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(3),
    )
    .await
    .unwrap_or_else(|f| panic!("{:?}", f.error));
    assert!(!ready.process.has_exited().unwrap());
    assert_eq!(ready.catalog.model().checkpoint, "model");
    ready.process.stop(Duration::from_secs(2)).unwrap();
    gone(&pid);
}
#[tokio::test]
async fn startup_failures_settle_owned_process() {
    for mode in ["changed", "changed_file", "exit", "hang", "busy"] {
        let (_f, request, pid) = fixture(mode);
        let failure = start(
            request,
            Arc::new(AtomicBool::new(false)),
            Duration::from_millis(400),
        )
        .await
        .err()
        .expect("must fail");
        assert!(failure.cleanup_confirmed, "{mode}");
        assert!(failure.unsettled.is_none());
        gone(&pid);
    }
}
#[tokio::test]
async fn occupied_port_and_pre_cancel_do_not_launch_or_adopt() {
    let (_f, mut request, pid) = fixture("ready");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    request.port = listener.local_addr().unwrap().port();
    let failure = start(
        request,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(1),
    )
    .await
    .err()
    .unwrap();
    assert!(failure.cleanup_confirmed);
    assert!(!pid.exists());
    assert!(listener.local_addr().is_ok());
    let (_f, request, pid) = fixture("ready");
    let failure = start(
        request,
        Arc::new(AtomicBool::new(true)),
        Duration::from_secs(1),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.error.code, ErrorCode::Cancelled);
    assert!(!pid.exists());
}

#[tokio::test]
async fn cancellation_after_spawn_reaps_child() {
    let (_f, request, pid) = fixture("hang");
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let path = pid.clone();
    let trigger = std::thread::spawn(move || {
        // CI may spend several seconds scheduling preflight. Cancel only after
        // the child has published its PID, never merely because time elapsed.
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while std::time::Instant::now() < deadline {
            if fs::read_to_string(&path)
                .ok()
                .and_then(|value| value.trim().parse::<u32>().ok())
                .is_some_and(|pid| pid > 0)
            {
                flag.store(true, Ordering::Release);
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    });
    let failure = start(request, cancel, Duration::from_secs(35))
        .await
        .err()
        .unwrap();
    assert!(
        trigger.join().unwrap(),
        "fixture child did not publish its PID"
    );
    assert_eq!(failure.error.code, ErrorCode::Cancelled);
    assert!(failure.cleanup_confirmed);
    assert!(pid.exists());
    gone(&pid);
}

mod exchange;

mod job;

mod api;

mod build;
