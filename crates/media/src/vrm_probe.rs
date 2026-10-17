//! Metadata-only VRM worker boundary; this does not certify that a rig can render.
use crate::{ProcessSpec, StopReason, run_process};
use serde::Deserialize;
use std::{
    ffi::OsString,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub use sungeki_project::{VrmInspection, VrmKind};
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VrmProbe {
    pub version: u32,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub inspection: VrmInspection,
    /// Untrusted model credit metadata, retained without interpreting permissions.
    pub credit: Option<serde_json::Value>,
}
impl VrmProbe {
    pub fn asset_probe(&self) -> sungeki_project::VrmAssetProbe {
        sungeki_project::VrmAssetProbe {
            sha256: self.sha256.clone(),
            byte_length: self.bytes,
            inspection: self.inspection.clone(),
        }
    }
}
#[derive(Debug)]
pub enum VrmProbeError {
    Io(std::io::Error),
    Invalid(String),
    Stopped(StopReason),
    Worker(String),
}
impl std::fmt::Display for VrmProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for VrmProbeError {}

pub struct VrmProbeConfig<'a> {
    pub node: &'a Path,
    pub worker: &'a Path,
    pub directory: &'a Path,
    pub timeout: Duration,
}

pub fn probe_vrm(
    config: &VrmProbeConfig<'_>,
    path: &Path,
    kind: VrmKind,
    cancel: &AtomicBool,
) -> Result<VrmProbe, VrmProbeError> {
    if cancel.load(Ordering::Acquire) {
        return Err(VrmProbeError::Stopped(StopReason::Cancelled));
    }
    if !path.is_absolute()
        || !config.node.is_absolute()
        || !config.worker.is_absolute()
        || !config.directory.is_absolute()
        || config.timeout.is_zero()
    {
        return Err(VrmProbeError::Invalid(
            "素材/ランタイム/作業先は絶対パス、制限時間は正数が必要です".into(),
        ));
    }
    let args: Vec<OsString> = vec![
        "--experimental-strip-types".into(),
        config.worker.into(),
        "--path".into(),
        path.into(),
        "--kind".into(),
        match kind {
            VrmKind::Model => "model",
            VrmKind::Motion => "motion",
        }
        .into(),
    ];
    let result = run_process(
        &ProcessSpec {
            program: config.node,
            args: &args,
            directory: config.directory,
            timeout: config.timeout,
            max_output_bytes: 1024 * 1024,
        },
        cancel,
    )
    .map_err(VrmProbeError::Io)?;
    if let Some(reason) = result.stopped {
        return Err(VrmProbeError::Stopped(reason));
    }
    if !result.status.success() {
        return Err(VrmProbeError::Worker(
            String::from_utf8_lossy(&result.stderr).into_owned(),
        ));
    }
    let probe = decode(&result.stdout, path, kind)?;
    if cancel.load(Ordering::Acquire) {
        return Err(VrmProbeError::Stopped(StopReason::Cancelled));
    }
    Ok(probe)
}

fn decode(bytes: &[u8], path: &Path, kind: VrmKind) -> Result<VrmProbe, VrmProbeError> {
    let p: VrmProbe =
        serde_json::from_slice(bytes).map_err(|e| VrmProbeError::Invalid(e.to_string()))?;
    if p.version != 1 || Path::new(&p.path) != path || p.inspection.kind != kind {
        return Err(VrmProbeError::Invalid(
            "VRM検査応答が要求と一致しません".into(),
        ));
    }
    p.asset_probe()
        .validate()
        .map_err(|e| VrmProbeError::Invalid(e.to_string()))?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_mismatched_and_malformed_worker_evidence() {
        let path = Path::new("/fixture/model.vrm");
        let valid = serde_json::json!({"version":1,"path":path,"bytes":100,"sha256":"a".repeat(64),
            "inspection":{"kind":"model","version":"1.0","expressions":["aa"]},"credit":null});
        let parse =
            |v: &serde_json::Value| decode(&serde_json::to_vec(v).unwrap(), path, VrmKind::Model);
        assert!(parse(&valid).is_ok());
        for (key, value) in [
            ("version", serde_json::json!(2)),
            ("path", serde_json::json!("/other")),
            ("bytes", serde_json::json!(0)),
            ("sha256", serde_json::json!("fake")),
        ] {
            let mut changed = valid.clone();
            changed[key] = value;
            assert!(parse(&changed).is_err());
        }
        for (key, value) in [
            ("kind", serde_json::json!("motion")),
            ("version", serde_json::json!("2.0")),
            ("expressions", serde_json::json!(["aa", "aa"])),
        ] {
            let mut changed = valid.clone();
            changed["inspection"][key] = value;
            assert!(parse(&changed).is_err());
        }
    }
}
