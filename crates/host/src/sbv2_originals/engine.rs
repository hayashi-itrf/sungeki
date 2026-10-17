//! Hash the explicitly configured engine/runtime manifest, without executing it.
use super::{file, invalid};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::AtomicBool};
use sungeki_protocol::ApiError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EngineFiles {
    /// Invoke through this venv/bin/python path to retain Python's venv discovery.
    pub executable: PathBuf,
    /// Explicit resolved binary inspected without following its final component.
    pub executable_target: PathBuf,
    /// Source, configuration and runtime dependencies keyed by stable logical name.
    /// The installation/launch owner must establish completeness of this list.
    pub files: BTreeMap<String, PathBuf>,
}
impl EngineFiles {
    pub fn revision(&self, cancel: &AtomicBool) -> Result<String, ApiError> {
        if self.files.len() > 65536
            || ![
                "server_fastapi.py",
                "config.py",
                "config.yml",
                "next_launcher.py",
            ]
            .iter()
            .all(|name| self.files.contains_key(*name))
            || self.files.keys().any(|key| {
                key.trim().is_empty() || key.len() > 1024 || key.chars().any(char::is_control)
            })
        {
            return Err(invalid("SBV2エンジンのファイル一覧が不正です"));
        }
        let (executable, environment) =
            super::python::inspect(&self.executable, &self.executable_target, cancel)?;
        let mut remaining = 8u64 * 1024 * 1024 * 1024;
        let mut measured = Vec::new();
        for (name, path) in &self.files {
            let measured_file =
                file::inspect_source(path, remaining.min(512 * 1024 * 1024), cancel)?;
            remaining -= measured_file.len;
            measured.push((name, measured_file.sha256));
        }
        let bytes = serde_json::to_vec(&(
            "vvm-sbv2-engine-files-v2",
            executable,
            environment,
            measured,
        ))
        .map_err(invalid)?;
        Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
    }
}
