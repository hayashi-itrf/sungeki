//! Checked command construction; spawning/resource ownership belongs to the host.
use super::{Measured, Paths, inspect, invalid};
use crate::original_file;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
};
use sungeki_protocol::ApiError;
use sungeki_speech::qwen::RuntimeIdentity;
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub engine_root: PathBuf,
    pub model_root: PathBuf,
    pub voice_root: PathBuf,
    pub work_root: PathBuf,
    pub executable: PathBuf,
    pub executable_target: PathBuf,
    pub launcher: PathBuf,
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
impl Binding {
    /// Port must be reserved by the owner and passed on stdin before executing.
    /// Complete dependency discovery and post-boot attestation are separate gates.
    pub fn launch_command(
        &self,
        port: u16,
        expected: &RuntimeIdentity,
        cancel: &AtomicBool,
    ) -> Result<Command, ApiError> {
        self.checked_command(port, expected, cancel)
            .map(|(command, _)| command)
    }
    pub fn checked_command(
        &self,
        port: u16,
        expected: &RuntimeIdentity,
        cancel: &AtomicBool,
    ) -> Result<(Command, Measured), ApiError> {
        let (command, measured) =
            self.measured_command(port, &expected.backend, &expected.model_id, cancel)?;
        measured.verify_files(expected)?;
        Ok((command, measured))
    }
    /// Inspect once for initial registration; no saved hashes exist at this point.
    pub(super) fn measured_command(
        &self,
        port: u16,
        backend: &str,
        model_id: &str,
        cancel: &AtomicBool,
    ) -> Result<(Command, Measured), ApiError> {
        if port < 1024 || backend != "official" {
            return Err(invalid());
        }
        let roots = [
            canonical(&self.engine_root)?,
            canonical(&self.model_root)?,
            canonical(&self.voice_root)?,
            canonical(&self.work_root)?,
        ];
        if roots.iter().any(|p| !p.is_dir())
            || roots[..3].iter().any(|p| roots[3].starts_with(p))
            || model_id != roots[1].to_str().ok_or_else(invalid)?
        {
            return Err(invalid());
        }
        let executable_target = canonical(&self.executable_target)?;
        let launcher = canonical(&self.launcher)?;
        let config = self
            .executable
            .parent()
            .and_then(Path::parent)
            .ok_or_else(invalid)?
            .join("pyvenv.cfg");
        for (name, path) in [
            ("api/main.py", roots[0].join("api/main.py")),
            ("next_launcher.py", launcher.clone()),
            ("python_executable", executable_target.clone()),
            ("pyvenv.cfg", canonical(&config)?),
        ] {
            if self.originals.engine.get(name) != Some(&path) {
                return Err(invalid());
            }
        }
        if self.originals.model != super::model_files::discover(&roots[1], cancel)? {
            return Err(invalid());
        }
        original_file::python::inspect(&self.executable, &executable_target, cancel)?;
        let measured = inspect(&self.originals, cancel)?;
        let mut command = Command::new(&self.executable);
        command.args(["-I", "-B"]).arg(launcher);
        for (flag, path) in [
            "--engine-root",
            "--model-root",
            "--voice-root",
            "--work-root",
        ]
        .iter()
        .zip(&roots)
        {
            command.arg(flag).arg(path);
        }
        command
            .args(["--port", &port.to_string(), "--listen-fd", "0"])
            .current_dir(&roots[3])
            .env_remove("PYTHONHOME")
            .env_remove("PYTHONPATH");
        Ok((command, measured))
    }
}
#[cfg(test)]
mod tests;
