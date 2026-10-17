use super::*;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
#[test]
fn binds_launch_files_and_execution_conditions_without_spawning() {
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
    let cancel = AtomicBool::new(false);
    let (_, measured) = b.measured_command("model", &cancel).unwrap();
    let expected = RuntimeIdentity {
        checkpoint: "model".into(),
        model_manifest_sha256: measured.model_manifest_sha256,
        codec_manifest_sha256: measured.codec_manifest_sha256,
        engine_manifest_sha256: measured.engine_manifest_sha256,
        voice_manifest_sha256: measured.voice_manifest_sha256,
        sample_rate: 48000,
        channels: 1,
    };
    let (command, _) = b.checked_command(&expected, &cancel).unwrap();
    assert_eq!(command.get_program(), b.executable.as_os_str());
    let args: Vec<_> = command.get_args().map(|a| a.to_str().unwrap()).collect();
    assert_eq!(&args[..2], &["-I", "-B"]);
    assert!(args.windows(2).any(|w| w == ["--listen-fd", "0"]));
    for mode in 0..5 {
        let mut changed = b.clone();
        match mode {
            0 => changed.device = "mps".into(),
            1 => changed.precision = "fp16".into(),
            2 => {
                changed.originals.engine.remove("next/bridge.py");
            }
            3 => changed.work_root = root.join("model"),
            _ => changed
                .originals
                .codec
                .insert("codec".into(), b.checkpoint.clone())
                .map(|_| ())
                .unwrap(),
        }
        assert!(
            changed.checked_command(&expected, &cancel).is_err(),
            "{mode}"
        );
    }
    assert!(
        b.checked_command(&expected, &AtomicBool::new(true))
            .is_err()
    );
    let cache = root.join("dependency-cache");
    fs::create_dir(&cache).unwrap();
    let mut cached = b.clone();
    cached.dependency_cache = Some(cache.clone());
    assert!(cached.measured_command("model", &cancel).is_err());
    fs::write(cache.join("weights"), "weights").unwrap();
    assert!(cached.measured_command("model", &cancel).is_err());
    cached
        .originals
        .engine
        .insert("hf-cache/weights".into(), cache.join("weights"));
    let (command, _) = cached.measured_command("model", &cancel).unwrap();
    assert!(command.get_args().any(|v| v == "--dependency-cache"));
    assert!(cached.checked_command(&expected, &cancel).is_err());
    std::os::unix::fs::symlink(cache.join("weights"), cache.join("alias")).unwrap();
    assert!(cached.measured_command("model", &cancel).is_err());
    fs::write(root.join("worker/audio.py"), "changed").unwrap();
    assert!(b.checked_command(&expected, &cancel).is_err());
}
