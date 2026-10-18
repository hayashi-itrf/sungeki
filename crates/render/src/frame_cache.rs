//! Successful static frames only, scoped to one job's captured originals/font.
//! No disk scan, cross-job reuse, or reuse of failed/overflowing render results.
use crate::{RenderError, execute::Runner, originals::write_new, plan::error};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::PathBuf};
use sungeki_project::Canvas;
use sungeki_scene::{Layer, Primitive};
#[path = "frame_batch.rs"]
mod batch;
mod identity;

#[derive(Debug, Default, Serialize)]
pub struct FrameCacheStats {
    pub rendered: u64,
    pub reused: u64,
}
#[derive(Default)]
pub(crate) struct FrameCache {
    raster: sungeki_raster::frame::Rasterizer,
    frames: HashMap<[u8; 32], usize>,
    pub stats: FrameCacheStats,
    deferred: bool,
    pending: Vec<batch::PendingFrame>,
}
// Ordered visual data only. IDs and interval bounds are provenance, not pixels.
// Match every primitive explicitly so a future time-dependent type cannot
// silently inherit this static key contract.
fn key(canvas: &Canvas, layers: &[&Layer], font_hash: &str) -> Result<[u8; 32], RenderError> {
    let visuals: Vec<_> = layers
        .iter()
        .map(|layer| {
            match &layer.primitive {
                Primitive::Vrm { .. } => {
                    return Err(error("VRM frameを解決してから描画してください"));
                }
                Primitive::Video { .. } => {
                    return Err(error("動画frameを解決してから描画してください"));
                }
                Primitive::Blur { .. }
                | Primitive::Text { .. }
                | Primitive::Shape { .. }
                | Primitive::Image { .. }
                | Primitive::Psd { .. } => {}
            }
            Ok((&layer.transform, &layer.primitive))
        })
        .collect::<Result<_, RenderError>>()?;
    let bytes = serde_json::to_vec(&("vvm-static-frame-v1", canvas, font_hash, visuals))?;
    Ok(Sha256::digest(bytes).into())
}
impl FrameCache {
    pub fn frame(
        &mut self,
        runner: &Runner<'_>,
        canvas: &Canvas,
        layers: &[&Layer],
        index: usize,
        frame_index: u64,
    ) -> Result<PathBuf, RenderError> {
        self.frame_with_resources(
            runner,
            canvas,
            layers,
            index,
            frame_index,
            &runner.directory.join("resources.json"),
        )
    }
    pub(crate) fn frame_with_resources(
        &mut self,
        runner: &Runner<'_>,
        canvas: &Canvas,
        layers: &[&Layer],
        index: usize,
        frame_index: u64,
        resources: &std::path::Path,
    ) -> Result<PathBuf, RenderError> {
        let input = runner.directory.join(format!("frame-{index}.json"));
        let time_us = canvas.fps.frame_time(frame_index);
        let packet =
            json!({"canvas":canvas,"frame_index":frame_index,"time_us":time_us,"layers":layers});
        write_new(&input, &serde_json::to_vec(&packet)?)?;
        let key = key(canvas, layers, &runner.runtime.visual()?.font_sha256)?;
        if let Some(original) = self.frames.get(&key) {
            write_new(
                &runner.directory.join(format!("report-{index}.json")),
                &serde_json::to_vec(&json!({
                    "frame_index":frame_index,"time_us":time_us,"font_sha256":runner.runtime.visual()?.font_sha256,
                    "layout_version":crate::CURRENT_LAYOUT_VERSION,"issues":[],"reused_frame":original
                }))?,
            )?;
            self.stats.reused += 1;
            return Ok(runner.directory.join(format!("frame-{original}.png")));
        }
        let png = runner.directory.join(format!("frame-{index}.png"));
        if self.deferred {
            self.pending.push(batch::PendingFrame {
                input,
                resources: resources.to_owned(),
                index,
                frame_index,
                key,
                layers: layers.iter().map(|layer| (*layer).clone()).collect(),
            });
            return Ok(png);
        }

        runner.check_active()?;
        let value = sungeki_raster::files::render(
            &mut self.raster,
            &input,
            resources,
            &runner.directory.join("font.ttc"),
            &runner.runtime.visual()?.font_sha256,
            &png,
            runner.flag,
        )
        .map_err(|failure| runner.raster_error(failure))?;
        runner.check_active()?;
        write_new(
            &runner.directory.join(format!("report-{index}.json")),
            &serde_json::to_vec(&value)?,
        )?;
        validate_report(&value, runner, canvas, layers, index, frame_index)?;
        self.frames.insert(key, index);
        self.stats.rendered += 1;
        Ok(png)
    }
}

fn validate_report(
    value: &Value,
    runner: &Runner<'_>,
    canvas: &Canvas,
    layers: &[&Layer],
    index: usize,
    frame_index: u64,
) -> Result<(), RenderError> {
    let time_us = canvas.fps.frame_time(frame_index);
    identity::validate(
        value,
        &runner.runtime.visual()?.font_sha256,
        frame_index,
        &serde_json::to_value(time_us)?,
        index,
    )?;
    let issues = crate::text_report::issues(&value["issues"], layers, canvas)?;
    if !issues.is_empty() {
        return Err(RenderError::TextLayout(issues));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sungeki_project::*;
    fn fixture() -> (Canvas, Vec<Layer>) {
        let p: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        let plan = sungeki_scene::ScenePlan::new(&p).unwrap();
        (plan.canvas, plan.layers)
    }
    #[test]
    fn pixel_inputs_and_order_change_keys_but_provenance_does_not() {
        let (canvas, mut layers) = fixture();
        let original = key(&canvas, &[&layers[0]], "font-a").unwrap();
        layers[0].clip_id = "other".into();
        layers[0].scene_id = None;
        layers[0].start_us = TimeUs(123);
        layers[0].end_us = TimeUs(456);
        assert_eq!(key(&canvas, &[&layers[0]], "font-a").unwrap(), original);
        assert_ne!(key(&canvas, &[&layers[0]], "font-b").unwrap(), original);
        for kind in 0..5 {
            let mut changed = layers[0].clone();
            match kind {
                0 => changed.transform.opacity = 0.5,
                1 => changed.transform.x += 1.,
                2 => {
                    if let Primitive::Psd { pose, .. } = &mut changed.primitive {
                        pose.selections[0].option = "/0/0".into();
                    }
                }
                3 => {
                    if let Primitive::Psd { asset_id, .. } = &mut changed.primitive {
                        *asset_id = "another-source".into();
                    }
                }
                _ => changed.transform.flip_x = Some(true),
            }
            assert_ne!(key(&canvas, &[&changed], "font-a").unwrap(), original);
            assert_ne!(
                key(&canvas, &[&changed, &layers[0]], "font-a").unwrap(),
                key(&canvas, &[&layers[0], &changed], "font-a").unwrap()
            );
        }
        let mut background = canvas.clone();
        background.background = "#123456".into();
        assert_ne!(key(&background, &[&layers[0]], "font-a").unwrap(), original);
    }
    #[test]
    fn invalid_native_inputs_never_enter_cache() {
        use crate::RenderRuntime;
        use std::{
            fs,
            sync::atomic::AtomicBool,
            time::{Duration, Instant},
        };
        let directory =
            std::env::temp_dir().join(format!("vvm-frame-cache-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let runtime = RenderRuntime {
            vrm: None,

            ffmpeg: "/unused".into(),
            ffprobe: "/unused".into(),
            font: "/unused".into(),
            font_sha256: "font-a".into(),
            deadline: Duration::from_secs(5),
        };
        let flag = AtomicBool::new(false);
        let runner = Runner {
            runtime: (&runtime).into(),
            directory: &directory,
            flag: &flag,
            end: Instant::now() + Duration::from_secs(5),
        };
        let (canvas, layers) = fixture();
        let mut cache = FrameCache::default();
        for index in 0..2 {
            assert!(
                cache
                    .frame(&runner, &canvas, &[&layers[0]], index, index as u64)
                    .is_err()
            );
            assert!(cache.frames.is_empty());
            assert_eq!(cache.stats.rendered + cache.stats.reused, 0);
        }
        fs::remove_dir_all(&directory).unwrap();
    }
    #[test]
    fn unresolved_video_never_enters_static_cache() {
        let (canvas, mut layers) = fixture();
        layers[0].primitive = Primitive::Video {
            asset_id: "video".into(),
            source_time_us: TimeUs(0),
        };
        assert!(key(&canvas, &[&layers[0]], "font").is_err());
        layers[0].primitive = Primitive::Video {
            asset_id: "video".into(),
            source_time_us: TimeUs(33333),
        };
        assert!(key(&canvas, &[&layers[0]], "font").is_err());
    }
}
