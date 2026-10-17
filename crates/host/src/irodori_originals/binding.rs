//! Bind inspected originals to explicit launcher arguments; never spawn here.
use super::{Measured, Paths, RuntimeIdentity, inspect, invalid};
use crate::original_file;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
};
use sungeki_protocol::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub engine_root: PathBuf,
    pub checkpoint: PathBuf,
    pub codec: PathBuf,
    pub registry: PathBuf,
    pub work_root: PathBuf,
    #[serde(default)]
    pub dependency_cache: Option<PathBuf>,
    pub executable: PathBuf,
    pub executable_target: PathBuf,
    pub launcher: PathBuf,
    pub device: String,
    pub precision: String,
    pub originals: Paths,
}
fn canonical(path: &Path) -> Result<PathBuf, ApiError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(invalid());
    }
    std::fs::canonicalize(path).map_err(ApiError::io)
}
fn required(
    files: &std::collections::BTreeMap<String, PathBuf>,
    name: &str,
    path: &Path,
) -> Result<(), ApiError> {
    if files.get(name) != Some(&canonical(path)?) {
        return Err(invalid());
    }
    Ok(())
}
impl Binding {
    /// The owner reserves the loopback socket and supplies it on stdin.
    pub fn checked_command(
        &self,
        expected: &RuntimeIdentity,
        cancel: &AtomicBool,
    ) -> Result<(Command, Measured), ApiError> {
        expected.validate().map_err(|_| invalid())?;
        let (command, measured) = self.measured_command(&expected.checkpoint, cancel)?;
        measured.verify_files(expected)?;
        Ok((command, measured))
    }
    pub fn measured_command(
        &self,
        checkpoint_id: &str,
        cancel: &AtomicBool,
    ) -> Result<(Command, Measured), ApiError> {
        if checkpoint_id.trim().is_empty()
            || checkpoint_id.len() > 4096
            || checkpoint_id.chars().any(char::is_control)
            || !["cpu", "mps", "cuda"].contains(&self.device.as_str())
            || !["fp32", "fp16", "bf16"].contains(&self.precision.as_str())
        {
            return Err(invalid());
        }
        let engine = canonical(&self.engine_root)?;
        let work = canonical(&self.work_root)?;
        let checkpoint = canonical(&self.checkpoint)?;
        let codec = canonical(&self.codec)?;
        let registry = canonical(&self.registry)?;
        let launcher = canonical(&self.launcher)?;
        let target = canonical(&self.executable_target)?;
        if !engine.is_dir() || !work.is_dir() {
            return Err(invalid());
        }
        for path in self
            .originals
            .model
            .values()
            .chain(self.originals.codec.values())
            .chain(self.originals.engine.values())
            .chain(self.originals.voice.values())
        {
            let original = canonical(path)?;
            if original.starts_with(&work) {
                return Err(invalid());
            }
        }
        for original in [
            &engine,
            checkpoint.parent().ok_or_else(invalid)?,
            codec.parent().ok_or_else(invalid)?,
            registry.parent().ok_or_else(invalid)?,
        ] {
            if work.starts_with(original) || original.starts_with(&work) {
                return Err(invalid());
            }
        }
        required(&self.originals.model, "checkpoint", &checkpoint)?;
        required(&self.originals.codec, "codec", &codec)?;
        required(&self.originals.voice, "registry", &registry)?;
        required(&self.originals.engine, "python_executable", &target)?;
        let config = self
            .executable
            .parent()
            .and_then(Path::parent)
            .ok_or_else(invalid)?
            .join("pyvenv.cfg");
        required(&self.originals.engine, "pyvenv.cfg", &config)?;
        required(
            &self.originals.engine,
            "irodori_tts/inference_runtime.py",
            &engine.join("irodori_tts/inference_runtime.py"),
        )?;
        let worker = launcher.parent().ok_or_else(invalid)?;
        if launcher.file_name().is_none_or(|name| name != "launch.py") {
            return Err(invalid());
        }
        for name in [
            "launch.py",
            "environment.py",
            "bridge.py",
            "audio.py",
            "server.py",
        ] {
            required(
                &self.originals.engine,
                &format!("next/{name}"),
                &worker.join(name),
            )?;
        }
        if let Some(cache) = &self.dependency_cache {
            let cache = canonical(cache)?;
            if !cache.is_dir() || cache.starts_with(&work) || work.starts_with(&cache) {
                return Err(invalid());
            }
            let mut pending = vec![cache.clone()];
            let mut count = 0;
            let mut files = 0;
            while let Some(directory) = pending.pop() {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err(invalid());
                }
                for entry in std::fs::read_dir(directory).map_err(ApiError::io)? {
                    let entry = entry.map_err(ApiError::io)?;
                    count += 1;
                    if count > 65536 {
                        return Err(invalid());
                    }
                    let kind = entry.file_type().map_err(ApiError::io)?;
                    if kind.is_dir() {
                        pending.push(entry.path());
                    } else if kind.is_file() {
                        files += 1;
                        let path = entry.path();
                        let relative = path.strip_prefix(&cache).map_err(|_| invalid())?;
                        required(
                            &self.originals.engine,
                            &format!("hf-cache/{}", relative.to_string_lossy()),
                            &path,
                        )?;
                    } else {
                        return Err(invalid());
                    }
                }
            }
            if files == 0 {
                return Err(invalid());
            }
        }
        let registry_before = super::registry::read(&registry, &self.originals.voice, cancel)?;
        original_file::python::inspect(&self.executable, &target, cancel)?;
        let mut measured = inspect(&self.originals, cancel)?;
        let registry_after = super::registry::read(&registry, &self.originals.voice, cancel)?;
        if registry_before.sha256 != registry_after.sha256 {
            return Err(ApiError::new(
                sungeki_protocol::ErrorCode::ExternalModification,
                "検査中にIrodori登録音声が変更されました",
            ));
        }
        // Execution choices influence generated samples and belong in identity.
        let bytes = serde_json::to_vec(&(
            "vvm-irodori-launch-v1",
            &measured.engine_manifest_sha256,
            checkpoint_id,
            &self.device,
            &self.precision,
            self.dependency_cache.is_some(),
        ))
        .map_err(|_| invalid())?;
        measured.engine_manifest_sha256 = format!("{:x}", Sha256::digest(bytes));
        let mut command = Command::new(&self.executable);
        command.args(["-I", "-B"]).arg(launcher);
        for (flag, path) in [
            ("--engine-root", &engine),
            ("--checkpoint", &checkpoint),
            ("--codec", &codec),
            ("--registry", &registry),
            ("--work-root", &work),
        ] {
            command.arg(flag).arg(path);
        }
        if let Some(cache) = &self.dependency_cache {
            command.arg("--dependency-cache").arg(canonical(cache)?);
        }
        command
            .args([
                "--checkpoint-id",
                checkpoint_id,
                "--listen-fd",
                "0",
                "--device",
                &self.device,
                "--precision",
                &self.precision,
            ])
            .current_dir(work)
            .env_remove("PYTHONHOME")
            .env_remove("PYTHONPATH");
        Ok((command, measured))
    }
}
#[cfg(test)]
mod tests;
