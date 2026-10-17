//! Bounded, owned VRM subprocess and validated image capture. No Project mutation.
pub use crate::vrm_output::VrmFrameOutput;
use crate::{
    RenderError, originals::write_new, plan::error, vrm_output, vrm_request::VrmRenderRequest,
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_media::{ProcessSpec, run_process};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrmRuntime {
    pub node: PathBuf,
    pub worker: PathBuf,
    pub work_root: PathBuf,
}
#[derive(Debug)]
pub struct VrmWorkerConfig<'a> {
    pub node: &'a Path,
    pub worker: &'a Path,
    pub work_root: &'a Path,
    pub timeout: Duration,
}
#[derive(Deserialize)]
struct ResultLine {
    status: String,
    directory: PathBuf,
}
/// destination must be a fresh private directory owned by the caller.
pub fn render_vrm_frames(
    request: &VrmRenderRequest,
    config: &VrmWorkerConfig<'_>,
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<VrmFrameOutput>, RenderError> {
    request.validate()?;
    crate::originals::cancelled(cancel)?;
    if [
        &config.node,
        &config.worker,
        &config.work_root,
        &destination,
    ]
    .iter()
    .any(|p| !p.is_absolute())
        || config.timeout.is_zero()
        || !std::fs::symlink_metadata(destination)?.is_dir()
        || std::fs::read_dir(destination)?.next().is_some()
    {
        return Err(error("VRM描画環境または専用作業先が不正です"));
    }
    let bytes = serde_json::to_vec(request)?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err(error("VRM描画要求が大きすぎます"));
    }
    let input = destination.join("request.json");
    write_new(&input, &bytes)?;
    let args: Vec<OsString> = vec![
        "--experimental-strip-types".into(),
        config.worker.into(),
        "--input".into(),
        input.into_os_string(),
        "--runtime-root".into(),
        config
            .work_root
            .parent()
            .ok_or_else(|| error("VRM作業先の親がありません"))?
            .as_os_str()
            .into(),
    ];
    let result = run_process(
        &ProcessSpec {
            program: config.node,
            args: &args,
            directory: destination,
            timeout: config.timeout,
            max_output_bytes: 1024 * 1024,
        },
        cancel,
    )?;
    if result.stopped == Some(sungeki_media::StopReason::Cancelled) {
        return Err(RenderError::Cancelled);
    }
    if !result.succeeded() {
        return Err(RenderError::Settled(format!(
            "VRM worker失敗: {:?}: {}",
            result.stopped,
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    let mut output = None;
    for line in result
        .stdout
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
    {
        let value: serde_json::Value = serde_json::from_slice(line)?;
        if value["type"] == "result" {
            if output.is_some() {
                return Err(error("VRM完了応答が重複しています"));
            }
            output = Some(serde_json::from_value::<ResultLine>(value)?);
        } else if value["type"] != "stage" {
            return Err(error("VRM応答の種類が不正です"));
        }
    }
    let output = output
        .filter(|o| o.status == "passed")
        .ok_or_else(|| error("VRM完了応答がありません"))?;
    vrm_output::capture(
        request,
        &output.directory,
        config.work_root,
        destination,
        cancel,
    )
}
