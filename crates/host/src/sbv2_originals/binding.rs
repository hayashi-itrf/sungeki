//! Configured launch context; metadata paths are compared, never opened directly.
use super::{EngineFiles, Measured, Paths, changed, inspect, invalid};
use std::{
    path::{Component, Path, PathBuf},
    sync::atomic::AtomicBool,
};
use sungeki_project::{Sbv2Identity, Sbv2Settings};
use sungeki_protocol::ApiError;
use sungeki_speech::sbv2::{Catalog, ModelConfig, canonical_endpoint};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub endpoint: String,
    pub working_directory: PathBuf,
    pub work_root: PathBuf,
    pub originals: Paths,
    pub engine: EngineFiles,
}
impl Binding {
    /// Build a command after checking the captured selection and original files.
    /// Does not spawn or claim process ownership; the owner must recheck after boot.
    pub fn launch_command(
        &self,
        catalog: &Catalog,
        settings: &Sbv2Settings,
        expected: &Sbv2Identity,
        cpu_only: bool,
        cancel: &AtomicBool,
    ) -> Result<std::process::Command, ApiError> {
        let port = self.managed_address()?.port();
        let endpoint = canonical_endpoint(&self.endpoint).map_err(invalid)?;
        self.inspect_selected(&endpoint, catalog, settings, expected, cancel)?;
        let launcher = self
            .engine
            .files
            .get("next_launcher.py")
            .ok_or_else(|| invalid("Sungeki Maker起動コードがありません"))?;
        let model_root = self
            .originals
            .model
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| invalid("SBV2モデルルートがありません"))?;
        let mut command = std::process::Command::new(&self.engine.executable);
        command
            .args(["-I", "-B"])
            .arg(launcher)
            .arg("--engine-root")
            .arg(&self.working_directory)
            .arg("--work-root")
            .arg(&self.work_root)
            .arg("--model-root")
            .arg(model_root)
            .arg("--port")
            .arg(port.to_string())
            .args(["--listen-fd", "0"])
            .arg("--device")
            .arg(if cpu_only { "cpu" } else { "auto" })
            .current_dir(&self.working_directory)
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONHOME");
        Ok(command)
    }
    pub fn managed_address(&self) -> Result<std::net::SocketAddrV4, ApiError> {
        let endpoint = canonical_endpoint(&self.endpoint).map_err(invalid)?;
        let port = endpoint
            .strip_prefix("http://127.0.0.1:")
            .and_then(|s| s.strip_suffix('/'))
            .and_then(|s| s.parse::<u16>().ok())
            .filter(|port| *port >= 1024)
            .ok_or_else(|| invalid("Sungeki Maker管理起動には127.0.0.1の1024以上のportが必要です"))?;
        Ok(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::LOCALHOST,
            port,
        ))
    }
    /// Blocks on file I/O; caller must own the process/resource lease and call on
    /// a blocking worker. This does NOT establish that the process loaded these files.
    pub fn inspect_selected(
        &self,
        endpoint: &str,
        catalog: &Catalog,
        settings: &Sbv2Settings,
        expected: &Sbv2Identity,
        cancel: &AtomicBool,
    ) -> Result<Measured, ApiError> {
        self.validate_catalog(endpoint, catalog, settings)?;
        expected.validate().map_err(invalid)?;
        if self.engine.revision(cancel)? != expected.engine_revision {
            return Err(changed(
                "SBV2エンジンの実ファイルが保存されたrevisionと一致しません",
            ));
        }
        let measured = inspect(&self.originals, cancel)?;
        measured.verify_files(expected)?;
        ModelConfig::parse(measured.config_text.as_bytes())
            .map_err(invalid)?
            .expected_audio(catalog, settings)
            .map_err(invalid)?;
        Ok(measured)
    }
    pub(super) fn validate_catalog(
        &self,
        endpoint: &str,
        catalog: &Catalog,
        settings: &Sbv2Settings,
    ) -> Result<(), ApiError> {
        if canonical_endpoint(&self.endpoint).map_err(invalid)?
            != canonical_endpoint(endpoint).map_err(invalid)?
        {
            return Err(changed("SBV2の設定済み接続先がジョブと一致しません"));
        }
        let cwd = normalized(&self.working_directory)?;
        for name in ["server_fastapi.py", "config.py", "config.yml"] {
            if self
                .engine
                .files
                .get(name)
                .map(|p| normalized(p))
                .transpose()?
                != Some(cwd.join(name))
            {
                return Err(invalid(
                    "SBV2の起動cwdとエンジンファイルの設定が一致しません",
                ));
            }
        }
        catalog.select(settings).map_err(invalid)?;
        let selected = &catalog.models()[&settings.model_id];
        let model = normalized(&self.originals.model)?;
        let config = normalized(&self.originals.config)?;
        let styles = normalized(&self.originals.style_vectors)?;
        let directory = model
            .parent()
            .ok_or_else(|| invalid("モデルの親がありません"))?;
        // Installed server loads both companion files beside the model, fixed names.
        if config != directory.join("config.json") || styles != directory.join("style_vectors.npy")
        {
            return Err(invalid(
                "SBV2のconfig/style_vectorsはモデルと同じフォルダに必要です",
            ));
        }
        let reported_model = normalized(&cwd.join(&selected.model_path))?;
        let reported_config = normalized(&cwd.join(&selected.config_path))?;
        if reported_model != model || reported_config != config {
            return Err(changed(
                "SBV2サーバーの選択原本が設定済みパスと一致しません",
            ));
        }
        let work = std::fs::canonicalize(normalized(&self.work_root)?).map_err(ApiError::io)?;
        if !work.is_dir() {
            return Err(invalid("SBV2作業先は既存フォルダが必要です"));
        }
        let engine = std::fs::canonicalize(&cwd).map_err(ApiError::io)?;
        if work.starts_with(&engine) || engine.starts_with(&work) {
            return Err(invalid("SBV2作業先をエンジン原本から分離してください"));
        }
        for original in self
            .engine
            .files
            .values()
            .chain(self.originals.text_dependencies.values())
            .chain([
                &self.originals.model,
                &self.originals.config,
                &self.originals.style_vectors,
            ])
        {
            let original = std::fs::canonicalize(original).map_err(ApiError::io)?;
            if original.starts_with(&work) || original.parent().is_some_and(|p| work.starts_with(p))
            {
                return Err(invalid("SBV2作業先に原本を配置できません"));
            }
        }
        Ok(())
    }
}
// Lexical comparison: no filesystem access to paths supplied by HTTP. Aliases
// through different symlink names are not treated as equivalent automatically.
fn normalized(path: &Path) -> Result<PathBuf, ApiError> {
    if !path.is_absolute() {
        return Err(invalid("SBV2の起動設定には絶対パスが必要です"));
    }
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Normal(_) => result.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                // Collapsing .. across a symlink can change the actual target.
                return Err(invalid("SBV2原本パスに親ディレクトリ参照は使えません"));
            }
            _ => return Err(invalid("未対応のSBV2原本パスです")),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
