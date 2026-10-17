use crate::{
    RenderError,
    originals::{cancelled, read_verified, write_new},
    plan::error,
    vrm_request::VrmRenderRequest,
};
use serde::Deserialize;
use std::{
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
#[derive(Deserialize)]
struct Input {
    sha256: String,
    bytes: u64,
    path: PathBuf,
}
#[derive(Deserialize)]
struct Inputs {
    model: Input,
    motion: Input,
}
#[derive(Deserialize)]
struct Frame {
    #[serde(default)]
    pose: std::collections::BTreeMap<String, [f64; 3]>,
    expressions: std::collections::BTreeMap<String, f64>,
    index: usize,
    time_us: u64,
    filename: String,
    bytes: u64,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    status: String,
    cleanup_passed: bool,
    remaining_processes: Vec<serde_json::Value>,
    request_version: u32,
    width: u32,
    height: u32,
    inputs: Inputs,
    frames: Vec<Frame>,
}
#[derive(Debug)]
pub struct VrmFrameOutput {
    pub path: PathBuf,
    pub probe: sungeki_project::ImageProbe,
}
fn read_report(path: &Path) -> Result<Report, RenderError> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.len() > 32 * 1024 * 1024 {
        return Err(error("VRMレポートの種類・容量が不正です"));
    }
    let mut bytes = vec![];
    Read::by_ref(&mut file)
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != meta.len() {
        return Err(error("VRMレポートが変更されました"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
pub(crate) fn capture(
    request: &VrmRenderRequest,
    output: &Path,
    work_root: &Path,
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<VrmFrameOutput>, RenderError> {
    if !output.is_absolute()
        || !fs::symlink_metadata(output)?.is_dir()
        || output.canonicalize()?.parent() != Some(work_root.canonicalize()?.as_path())
        || !output
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("render-") && !s.contains('.'))
    {
        return Err(error("VRM出力の保存先が不正です"));
    }
    let report = read_report(&output.join("report.json"))?;
    if report.status != "passed"
        || !report.cleanup_passed
        || !report.remaining_processes.is_empty()
        || report.request_version != request.version
        || report.width != request.width
        || report.height != request.height
        || report.frames.len() != request.frames.len()
    {
        return Err(error("VRM描画完了の証拠が要求と一致しません"));
    }
    for (expected, actual) in [
        (&request.model, &report.inputs.model),
        (&request.motion, &report.inputs.motion),
    ] {
        if expected.path != actual.path
            || expected.sha256 != actual.sha256
            || expected.byte_length != actual.bytes
        {
            return Err(error("VRM原本の応答が要求と一致しません"));
        }
    }
    let mut frames = vec![];
    let mut total = 0u64;
    for (index, (frame, expected)) in report.frames.iter().zip(&request.frames).enumerate() {
        cancelled(cancel)?;
        if frame.index != index
            || frame.time_us != expected.time_us.0
            || frame.expressions != expected.expressions
            || frame.pose != expected.pose
            || frame.filename != format!("frame-{index:06}.png")
        {
            return Err(error("VRM画像の順序・時刻・名前が不正です"));
        }
        total = total
            .checked_add(frame.bytes)
            .ok_or_else(|| error("VRM画像容量が不正です"))?;
        if total > 1024 * 1024 * 1024 {
            return Err(error("VRM画像が1GiBを超えています"));
        }
        let bytes = read_verified(
            &output.join(&frame.filename),
            &frame.sha256,
            Some(frame.bytes),
        )?;
        let path = destination.join(format!("captured-{index:06}.png"));
        write_new(&path, &bytes)?;
        let probe = sungeki_assets::probe_image(&path, Default::default()).map_err(error)?;
        if probe.sha256 != frame.sha256
            || probe.byte_length != frame.bytes
            || probe.width != request.width
            || probe.height != request.height
            || !probe.has_alpha
            || probe.orientation != 1
            || probe.format != sungeki_project::ImageFormat::Png
        {
            return Err(error("VRM画像の内容が要求と一致しません"));
        }
        frames.push(VrmFrameOutput { path, probe });
    }
    cancelled(cancel)?;
    Ok(frames)
}

#[cfg(test)]
#[path = "vrm_output_tests.rs"]
mod tests;
