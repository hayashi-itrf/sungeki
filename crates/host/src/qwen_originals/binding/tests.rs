use super::*;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
#[test]
fn verifies_command_originals_venv_and_launch_roots_without_spawning() {
    let root = std::env::temp_dir().join(format!("qwen-binding-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    struct Clean(PathBuf);
    impl Drop for Clean {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _clean = Clean(root.clone());
    for name in ["engine/api", "model", "voice", "work", "venv/bin"] {
        fs::create_dir_all(root.join(name)).unwrap();
    }
    for (name, content) in [
        ("engine/api/main.py", "main"),
        ("model/config.json", "{}"),
        ("voice/mapping.json", "{}"),
        ("launch.py", "launcher"),
        ("python-real", "binary"),
        ("venv/pyvenv.cfg", "include-system-site-packages = false\n"),
    ] {
        fs::write(root.join(name), content).unwrap();
    }
    fs::set_permissions(root.join("python-real"), fs::Permissions::from_mode(0o700)).unwrap();
    symlink(root.join("python-real"), root.join("venv/bin/python")).unwrap();
    let binding = Binding {
        engine_root: root.join("engine"),
        model_root: root.join("model"),
        voice_root: root.join("voice"),
        work_root: root.join("work"),
        executable: root.join("venv/bin/python"),
        executable_target: root.join("python-real"),
        launcher: root.join("launch.py"),
        originals: Paths {
            model: BTreeMap::from([("config.json".into(), root.join("model/config.json"))]),
            engine: [
                ("api/main.py", "engine/api/main.py"),
                ("next_launcher.py", "launch.py"),
                ("python_executable", "python-real"),
                ("pyvenv.cfg", "venv/pyvenv.cfg"),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), root.join(v)))
            .collect(),
            voice: BTreeMap::from([("mapping".into(), root.join("voice/mapping.json"))]),
        },
    };
    let flag = AtomicBool::new(false);
    let measured = inspect(&binding.originals, &flag).unwrap();
    let identity = RuntimeIdentity {
        backend: "official".into(),
        model_id: binding.model_root.to_str().unwrap().into(),
        model_manifest_sha256: measured.model_manifest_sha256,
        engine_manifest_sha256: measured.engine_manifest_sha256,
        voice_manifest_sha256: measured.voice_manifest_sha256,
        sample_rate: 24000,
        channels: 1,
    };
    let command = binding.launch_command(8880, &identity, &flag).unwrap();
    assert_eq!(command.get_program(), binding.executable.as_os_str());
    assert_eq!(command.get_current_dir(), Some(binding.work_root.as_path()));
    let args: Vec<_> = command.get_args().map(|s| s.to_str().unwrap()).collect();
    assert_eq!(&args[..2], &["-I", "-B"]);
    assert_eq!(
        &args[args.len() - 4..],
        &["--port", "8880", "--listen-fd", "0"]
    );
    assert!(binding.launch_command(80, &identity, &flag).is_err());
    assert!(
        binding
            .launch_command(8880, &identity, &AtomicBool::new(true))
            .is_err()
    );
    let mut wrong = binding.clone();
    wrong.work_root = wrong.model_root.clone();
    assert!(wrong.launch_command(8880, &identity, &flag).is_err());
    let mut wrong = identity.clone();
    wrong.model_id = "other".into();
    assert!(binding.launch_command(8880, &wrong, &flag).is_err());
    // An omitted newly added shard must fail even when all listed hashes match.
    let shard = binding.model_root.join("model-00002.safetensors");
    fs::write(&shard, "additional weights").unwrap();
    assert!(binding.launch_command(8880, &identity, &flag).is_err());
    fs::remove_file(shard).unwrap();
    assert!(binding.launch_command(8880, &identity, &flag).is_ok());
    fs::write(&binding.launcher, "changed!").unwrap();
    assert!(binding.launch_command(8880, &identity, &flag).is_err());
}
