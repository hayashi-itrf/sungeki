use super::{Engine, error};
use std::{
    fs,
    net::TcpListener,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    process::{Command, Stdio},
};
use sungeki_protocol::{ApiError, RuntimeSettings};
pub(super) fn command(
    engine: Engine,
    endpoint: &str,
    binary: &Path,
    settings: &RuntimeSettings,
) -> Result<Option<Command>, ApiError> {
    let authority = endpoint
        .strip_prefix("http://")
        .and_then(|s| s.strip_suffix('/'))
        .ok_or_else(|| error("接続先が不正です"))?;
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| error("自動起動にはポートの指定が必要です"))?;
    let port: u16 = port.parse().map_err(error)?;
    if host != "127.0.0.1" || port < 1024 {
        return Err(error(
            "自動起動には127.0.0.1と1024以上のポートを指定してください",
        ));
    }
    if engine == Engine::Coeiroink && port != 50032 {
        return Err(error("COEIROINKの自動起動はポート50032を使用します"));
    }
    // Probe availability without adopting an unknown process. Official engines
    // cannot inherit a listening FD; they must bind themselves after launch.
    let listener = match TcpListener::bind((host, port)) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => return Ok(None),
        Err(e) => return Err(error(e)),
    };
    if !binary.is_absolute()
        || !binary.is_file()
        || fs::metadata(binary).map_err(error)?.permissions().mode() & 0o111 == 0
    {
        return Err(error(
            "音声エンジンの実行ファイルを絶対パスで設定してください",
        ));
    }
    let root = settings
        .runtime_root
        .as_ref()
        .ok_or_else(|| error("自動起動には外部ディスクの保存フォルダーを設定してください"))?;
    let root = crate::runtime_root::checked(root)
        .map_err(error)?
        .join("http-engines")
        .join(engine.name());
    fs::create_dir_all(&root).map_err(error)?;
    let temporary = root.join("tmp");
    fs::create_dir_all(&temporary).map_err(error)?;
    let log = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(root.join(format!("engine-{}.log", uuid::Uuid::new_v4())))
        .map_err(error)?;
    let mut command = Command::new(binary);
    command
        .current_dir(binary.parent().unwrap())
        .env("TMPDIR", temporary)
        .stdout(Stdio::from(log.try_clone().map_err(error)?))
        .stderr(Stdio::from(log));
    if engine != Engine::Coeiroink {
        command
            .args([
                "--host",
                host,
                "--port",
                &port.to_string(),
                "--setting_file",
            ])
            .arg(root.join("settings.yaml"))
            .arg("--preset_file")
            .arg(root.join("presets.yaml"));
        command.arg(if engine == Engine::Aivis {
            "--disable_sentry"
        } else {
            "--disable_mutable_api"
        });
    }
    drop(listener);
    Ok(Some(command))
}
