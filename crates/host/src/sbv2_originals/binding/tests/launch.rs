use super::*;
#[test]
fn checked_command_preserves_venv_invocation_and_separates_arguments() {
    let (_fixture, binding, catalog, settings, identity) = setup();
    for cpu in [true, false] {
        let command = binding
            .launch_command(&catalog, &settings, &identity, cpu, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(command.get_program(), binding.engine.executable.as_os_str());
        assert_ne!(
            command.get_program(),
            binding.engine.executable_target.as_os_str()
        );
        let args: Vec<_> = command
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "-I".to_string(),
                "-B".into(),
                binding.engine.files["next_launcher.py"]
                    .to_str()
                    .unwrap()
                    .into(),
                "--engine-root".into(),
                binding.working_directory.to_str().unwrap().into(),
                "--work-root".into(),
                binding.work_root.to_str().unwrap().into(),
                "--model-root".into(),
                binding
                    .originals
                    .model
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .into(),
                "--port".into(),
                "5050".into(),
                "--listen-fd".into(),
                "0".into(),
                "--device".into(),
                if cpu { "cpu".into() } else { "auto".into() }
            ]
        );
        assert_eq!(
            command.get_current_dir(),
            Some(binding.working_directory.as_path())
        );
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == "PYTHONPATH" && value.is_none())
        );
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == "PYTHONHOME" && value.is_none())
        );
    }
}
#[test]
fn changed_environment_or_unsupported_managed_endpoint_does_not_build_a_command() {
    let (_fixture, mut binding, catalog, settings, identity) = setup();
    for endpoint in [
        "http://127.0.0.1:80",
        "http://127.0.0.2:5050",
        "http://[::1]:5050",
    ] {
        binding.endpoint = endpoint.into();
        assert!(
            binding
                .launch_command(
                    &catalog,
                    &settings,
                    &identity,
                    true,
                    &AtomicBool::new(false)
                )
                .is_err()
        );
    }
    binding.endpoint = "http://127.0.0.1:5050".into();
    std::fs::write(
        binding
            .engine
            .executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("pyvenv.cfg"),
        b"include-system-site-packages = false\nhome = /changed\n",
    )
    .unwrap();
    assert!(
        binding
            .launch_command(
                &catalog,
                &settings,
                &identity,
                true,
                &AtomicBool::new(false)
            )
            .is_err()
    );
}

#[test]
fn work_directory_cannot_alias_or_contain_originals() {
    let (_fixture, mut binding, catalog, settings, identity) = setup();
    let original = binding.work_root.clone();
    for work in [
        binding.working_directory.clone(),
        binding.working_directory.parent().unwrap().to_path_buf(),
    ] {
        binding.work_root = work;
        assert!(
            binding
                .launch_command(
                    &catalog,
                    &settings,
                    &identity,
                    true,
                    &AtomicBool::new(false)
                )
                .is_err()
        );
    }
    let alias = original.join("alias");
    std::os::unix::fs::symlink(&binding.working_directory, &alias).unwrap();
    binding.work_root = alias;
    assert!(
        binding
            .launch_command(
                &catalog,
                &settings,
                &identity,
                true,
                &AtomicBool::new(false)
            )
            .is_err()
    );
}
