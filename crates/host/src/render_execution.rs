//! Shared execution and verification, with an explicit preview purpose.
use crate::render_record::{Input, invalid, render_error};
use std::sync::atomic::AtomicBool;
use sungeki_protocol::{ApiError, RenderOutput, RenderPurpose};
pub(super) fn run(
    input: &Input,
    flag: &AtomicBool,
    settled: &mut bool,
    progress: &mut dyn FnMut(sungeki_render::RenderProgress),
) -> Result<(RenderOutput, Vec<u8>), ApiError> {
    let map_error = |error| {
        *settled = matches!(
            &error,
            sungeki_render::RenderError::Settled(_) | sungeki_render::RenderError::Cancelled
        );
        render_error(error)
    };
    let result = match input.purpose {
        RenderPurpose::Final => {
            let plan = crate::render_api::plan(&input.project)?;
            let artifact = sungeki_render::render_staged(
                &plan,
                &input.runtime,
                &input.directory,
                flag,
                progress,
            )
            .map_err(map_error)?;
            record(&artifact, serde_json::to_vec(&artifact).map_err(invalid)?)
        }
        RenderPurpose::EditingPreview => {
            let plan = sungeki_render::PreviewPlan::new(&input.project, |id| {
                crate::render_api::check_speech(&input.project, id)
            })
            .map_err(render_error)?;
            let artifact = plan
                .render_staged(&input.runtime, &input.directory, flag, progress)
                .map_err(map_error)?;
            record(
                artifact.media(),
                serde_json::to_vec(&artifact).map_err(invalid)?,
            )
        }
    };
    *settled = true;
    result
}
fn record(
    artifact: &sungeki_render::RenderArtifact,
    report: Vec<u8>,
) -> Result<(RenderOutput, Vec<u8>), ApiError> {
    Ok((
        RenderOutput {
            path: artifact
                .path
                .to_str()
                .ok_or_else(|| invalid("成果物パスが不正です"))?
                .into(),
            sha256: artifact.sha256.clone(),
            byte_length: artifact.byte_length,
            frame_count: artifact.frame_count,
            font_sha256: artifact.font_sha256.clone(),
            layout_version: artifact.layout_version,
        },
        report,
    ))
}
