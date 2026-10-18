//! Private staging and captured visual resources, shared by movie and PNG output.
use crate::{RenderError, RenderPlan, RenderRuntime, execute::Runner, originals, plan::error};
use std::{
    fs,
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

pub(crate) fn runner<'a>(
    runtime: &'a RenderRuntime,
    directory: &'a Path,
    flag: &'a AtomicBool,
) -> Result<Runner<'a>, RenderError> {
    if !directory.is_absolute()
        || !fs::symlink_metadata(directory)?.is_dir()
        || fs::read_dir(directory)?.next().is_some()
        || runtime.deadline.is_zero()
        || runtime.deadline > Duration::from_secs(86400)
        || [&runtime.ffmpeg, &runtime.ffprobe, &runtime.font]
            .iter()
            .any(|p| !p.is_absolute())
    {
        return Err(error("実行環境または空の専用作業ディレクトリが不正です"));
    }
    originals::cancelled(flag)?;
    Ok(Runner {
        runtime: runtime.into(),
        directory,
        flag,
        end: Instant::now() + runtime.deadline,
    })
}

pub(crate) fn visuals(
    plan: &RenderPlan,
    runner: &Runner<'_>,
) -> Result<crate::prepared_visuals::PreparedVisuals, RenderError> {
    let resources = originals::capture(plan, runner)?;
    originals::write_new(
        &runner.directory.join("font.ttc"),
        &originals::read_verified(
            &runner.runtime.visual()?.font,
            &runner.runtime.visual()?.font_sha256,
            None,
        )?,
    )?;
    fs::create_dir(runner.directory.join("psd-cache"))?;
    let prepared = crate::prepared_visuals::PreparedVisuals::new(plan, runner, resources)?;
    originals::write_new(
        &runner.directory.join("resources.json"),
        &serde_json::to_vec(&prepared.resources)?,
    )?;
    Ok(prepared)
}
