//! Fade the final composition (including unscened visuals) to canvas background.
//! A topmost solid layer is equivalent to blending the completed opaque frame.
use crate::{Layer, Primitive, first_frame_at_or_after};
use sungeki_project::*;

pub(crate) fn append(project: &Project, layers: &mut Vec<Layer>) -> Result<(), ValidationError> {
    let fps = &project.canvas.fps;
    for scene in &project.scenes {
        let Some(fade) = &scene.fade else { continue };
        let end = scene.start_us.0 + scene.duration_us.0;
        for (start, stop) in [
            (scene.start_us.0, scene.start_us.0 + fade.in_us.0),
            (end - fade.out_us.0, end),
        ] {
            if start == stop {
                continue;
            }
            let first = first_frame_at_or_after(fps, TimeUs(start)).unwrap();
            let last = first_frame_at_or_after(fps, TimeUs(stop)).unwrap();
            if last - first + layers.len() as u64 > 100_000 {
                return Err(ValidationError::new(
                    "シーンフェードの描画区間が100,000を超えています".into(),
                ));
            }
            for index in first..last {
                let time = fps.frame_time(index).unwrap();
                let opacity =
                    1. - fade.factor(TimeUs(time.0 - scene.start_us.0), scene.duration_us);
                layers.push(Layer {
                    // Provenance only; this is not an editable Project clip.
                    clip_id: scene.id.clone(),
                    scene_id: Some(scene.id.clone()),
                    start_us: time,
                    end_us: TimeUs(fps.frame_time(index + 1).unwrap().0.min(stop)),
                    transform: Transform {
                        motion: None,
                        x: 0.,
                        y: 0.,
                        width: project.canvas.width as f64,
                        height: project.canvas.height as f64,
                        opacity,
                        fade: None,
                        flip_x: None,
                    },
                    primitive: Primitive::Shape {
                        shape: ShapeKind::Rectangle,
                        fill: project.canvas.background.clone(),
                    },
                });
            }
        }
    }
    Ok(())
}
