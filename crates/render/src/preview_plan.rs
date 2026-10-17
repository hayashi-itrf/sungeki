//! Lower-resolution evaluation; the immutable Project and source media stay intact.
use crate::{RenderError, RenderPlan};
use sungeki_project::Project;
use sungeki_scene::{Primitive, ScenePlan};
const MAX_EDGE: u32 = 960;
/// Preview evidence is deliberately a different type from a formal output.
#[cfg(unix)]
#[derive(Debug, serde::Serialize)]
pub struct PreviewArtifact {
    purpose: &'static str,
    source_dimensions: (u32, u32),
    dimensions: (u32, u32),
    media: crate::RenderArtifact,
}
#[cfg(unix)]
impl PreviewArtifact {
    pub fn media(&self) -> &crate::RenderArtifact {
        &self.media
    }
}
#[derive(Debug)]
pub struct PreviewPlan {
    render: RenderPlan,
    source_dimensions: (u32, u32),
}
impl PreviewPlan {
    pub fn new(
        project: &Project,
        check_speech: impl Fn(&str) -> Result<(), RenderError>,
    ) -> Result<Self, RenderError> {
        let mut render = RenderPlan::new(project, check_speech)?;
        let source_dimensions = (render.scene.canvas.width, render.scene.canvas.height);
        shrink(&mut render.scene);
        Ok(Self {
            render,
            source_dimensions,
        })
    }
    pub fn dimensions(&self) -> (u32, u32) {
        (
            self.render.scene.canvas.width,
            self.render.scene.canvas.height,
        )
    }
    pub fn source_dimensions(&self) -> (u32, u32) {
        self.source_dimensions
    }
    /// Host integration must keep this artifact separate from formal publications.
    #[cfg(unix)]
    pub fn render_staged(
        &self,
        runtime: &crate::RenderRuntime,
        directory: &std::path::Path,
        flag: &std::sync::atomic::AtomicBool,
        progress: &mut dyn FnMut(crate::RenderProgress),
    ) -> Result<PreviewArtifact, RenderError> {
        let media = crate::render_staged(&self.render, runtime, directory, flag, progress)?;
        Ok(PreviewArtifact {
            purpose: "editing_preview",
            source_dimensions: self.source_dimensions(),
            dimensions: self.dimensions(),
            media,
        })
    }
}
fn shrink(scene: &mut ScenePlan) {
    let (width, height) = (scene.canvas.width, scene.canvas.height);
    if width.max(height) <= MAX_EDGE {
        return;
    }
    let requested = f64::from(MAX_EDGE) / f64::from(width.max(height));
    let even = |value: u32| ((f64::from(value) * requested / 2.).floor() as u32 * 2).max(2);
    let (out_width, out_height) = (even(width), even(height));
    // Uniform scale plus subpixel padding avoids stretching nonstandard canvases.
    let scale =
        (f64::from(out_width) / f64::from(width)).min(f64::from(out_height) / f64::from(height));
    let pad_x = (f64::from(out_width) - f64::from(width) * scale) / 2.;
    let pad_y = (f64::from(out_height) - f64::from(height) * scale) / 2.;
    scene.canvas.width = out_width;
    scene.canvas.height = out_height;
    for layer in &mut scene.layers {
        let transform = &mut layer.transform;
        transform.x = transform.x * scale + pad_x;
        transform.y = transform.y * scale + pad_y;
        transform.width *= scale;
        transform.height *= scale;
        if let Some(motion) = &mut transform.motion {
            motion.end_x = motion.end_x * scale + pad_x;
            motion.end_y = motion.end_y * scale + pad_y;
        }
        match &mut layer.primitive {
            Primitive::Text {
                font_size, outline, ..
            } => {
                // The rasterizer requires at least one pixel; subpixel text must
                // not abort a preview of an otherwise renderable source project.
                *font_size = (*font_size * scale).max(1.);
                if let Some(outline) = outline {
                    outline.width = ((f64::from(outline.width) * scale).round() as u32).max(1);
                }
            }
            Primitive::Blur { sigma } => *sigma *= scale,
            Primitive::Vrm { width, height, .. } => {
                *width = (f64::from(*width) * scale).ceil().max(1.) as u32;
                *height = (f64::from(*height) * scale).ceil().max(1.) as u32;
            }
            Primitive::Image { .. }
            | Primitive::Video { .. }
            | Primitive::Psd { .. }
            | Primitive::Shape { .. } => {}
        }
    }
}
#[cfg(test)]
#[path = "preview_plan_tests.rs"]
mod tests;
