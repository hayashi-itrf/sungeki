use crate::{Command, dialogue_template::rect};
use sungeki_project::*;

pub(crate) fn visual(
    p: &Project,
    id: &str,
    track: &str,
    scene: &str,
    start: TimeUs,
    duration: TimeUs,
    content: ClipContent,
    slot: usize,
    flip: bool,
) -> Result<Clip, ValidationError> {
    let mut c = Clip {
        id: id.into(),
        track_id: track.into(),
        scene_id: Some(scene.into()),
        start_us: start,
        duration_us: duration,
        content,
        transform: rect(0., 0., p.canvas.width as f64, p.canvas.height as f64),
    };
    c.transform.flip_x = Some(flip);
    c.transform = if matches!(c.content, ClipContent::Vrm { .. }) {
        let mut t = if p.canvas.width == 1080 {
            rect(215., 760., 650., 680.)
        } else {
            rect(if slot == 1 { 20. } else { 1550. }, 140., 350., 650.)
        };
        t.flip_x = Some(flip);
        t
    } else if p.canvas.width == 1080 {
        crate::build_portrait::fit(p, &c, slot == 0)?
    } else {
        crate::dialogue_geometry::fit(p, &c, slot)?
    };
    Ok(c)
}

pub(crate) fn standing(
    role: &BuildRole,
    pose: Option<&PsdPose>,
    sprite_pose: Option<&str>,
) -> (ClipContent, bool) {
    match &role.standing {
        BuildStanding::Vrm { standing, flip_x } => (
            ClipContent::Vrm {
                standing: standing.clone(),
            },
            *flip_x,
        ),
        BuildStanding::Sprite {
            sprites,
            pose,
            flip_x,
        } => (
            ClipContent::Sprite {
                sprites: sprites.clone(),
                pose: sprite_pose.map(str::to_owned).or_else(|| pose.clone()),
                character_id: Some(role.character.id.clone()),
            },
            *flip_x,
        ),
        BuildStanding::Image { asset_id, flip_x } => (
            ClipContent::Image {
                asset_id: asset_id.clone(),
            },
            *flip_x,
        ),
        BuildStanding::Psd {
            asset_id,
            pose: base,
            speech,
            flip_x,
        } => (
            ClipContent::Psd {
                asset_id: asset_id.clone(),
                pose: pose.unwrap_or(base).clone(),
                speech: Some(speech.clone()),
            },
            *flip_x,
        ),
    }
}

pub(crate) fn diagram_rect(p: &Project, asset_id: &str) -> Result<Transform, ValidationError> {
    let probe = &p
        .assets
        .iter()
        .find(|a| a.id == asset_id)
        .ok_or_else(|| ValidationError::new("説明素材が見つかりません".into()))?
        .probe;
    let (w, h) = match probe {
        AssetProbe::Image(probe) if (5..=8).contains(&probe.orientation) => {
            (probe.height as f64, probe.width as f64)
        }
        AssetProbe::Image(probe) => (probe.width as f64, probe.height as f64),
        AssetProbe::Video(probe) => {
            let g = VideoDisplayGeometry::new(&probe.metadata.video)
                .map_err(|e| ValidationError::new(e.into()))?;
            (g.width as f64, g.height as f64)
        }
        _ => {
            return Err(ValidationError::new(
                "説明素材は画像または動画が必要です".into(),
            ));
        }
    };
    let (x, y, width, height) = if p.canvas.width == 1080 {
        (60., 90., 960., 650.)
    } else {
        (390., 100., 1140., 650.)
    };
    let scale = (width / w).min(height / h);
    Ok(rect(
        x + (width - w * scale) / 2.,
        y + (height - h * scale) / 2.,
        w * scale,
        h * scale,
    ))
}

pub(crate) fn card(
    p: &Project,
    scene_id: &str,
    clip_ids: &[String; 2],
    track: &str,
    title: &str,
    text: String,
    start: u64,
    duration: u64,
    credits: bool,
) -> Vec<Command> {
    let mut commands = vec![Command::AddScene {
        scene: Scene {
            id: scene_id.into(),
            name: title.into(),
            start_us: TimeUs(start),
            duration_us: TimeUs(duration),
            fade: None,
        },
    }];
    let contents = [
        (
            ClipContent::Shape {
                shape: ShapeKind::Rectangle,
                fill: "#ffffff".into(),
            },
            rect(0., 0., p.canvas.width as f64, p.canvas.height as f64),
        ),
        (
            ClipContent::Text {
                outline: None,
                layout: Some(SubtitleLayout {
                    alignment: TextAlignment::Left,
                    wrap: true,
                }),
                text,
                font_size: if credits { 40. } else { 64. },
                color: "#111111".into(),
            },
            if p.canvas.width == 1080 {
                if credits {
                    rect(80., 180., 920., 1560.)
                } else {
                    rect(80., 480., 920., 700.)
                }
            } else if credits {
                rect(160., 120., 1600., 880.)
            } else {
                rect(160., 150., 1600., 500.)
            },
        ),
    ];
    for (id, (content, transform)) in clip_ids.iter().zip(contents) {
        commands.push(Command::AddClip {
            clip: Clip {
                id: id.clone(),
                track_id: track.into(),
                scene_id: Some(scene_id.into()),
                start_us: TimeUs(start),
                duration_us: TimeUs(duration),
                content,
                transform,
            },
        });
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn title_and_credit_cards_wrap_without_rewriting_source_text() {
        for (width, height) in [(1920, 1080), (1080, 1920)] {
            let mut project = Project::empty("project".into(), "test".into());
            project.canvas.width = width;
            project.canvas.height = height;
            for credits in [false, true] {
                let text = "長いタイトル・日本語とEnglishを省略せず表示する\n明示改行も保持";
                let commands = card(
                    &project,
                    "scene",
                    &["background".into(), "text".into()],
                    "track",
                    "scene",
                    text.into(),
                    0,
                    3_000_000,
                    credits,
                );
                let Command::AddClip { clip } = commands.last().unwrap() else {
                    panic!()
                };
                let ClipContent::Text {
                    text: saved,
                    layout,
                    ..
                } = &clip.content
                else {
                    panic!()
                };
                assert_eq!(saved, text);
                assert_eq!(
                    layout,
                    &Some(SubtitleLayout {
                        alignment: TextAlignment::Left,
                        wrap: true
                    })
                );
                assert!(clip.transform.x + clip.transform.width <= width as f64);
            }
        }
    }
    #[test]
    fn video_diagram_placement_uses_the_same_rotated_square_pixel_geometry() {
        let mut p: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json"))
                .unwrap();
        p.canvas.width = 1920;
        p.canvas.height = 1080;
        let asset = &mut p.assets[0];
        let AssetProbe::Video(probe) = &mut asset.probe else {
            panic!()
        };
        probe.metadata.video.width = 320;
        probe.metadata.video.height = 180;
        probe.metadata.video.sample_aspect_ratio = Some(MediaRatio {
            numerator: 2,
            denominator: 1,
        });
        probe.metadata.video.display_matrix = None;
        probe.metadata.video.rotation_degrees = Some(90.);
        let id = asset.id.clone();
        let rect = diagram_rect(&p, &id).unwrap();
        assert_eq!(rect.height, 650.);
        assert!((rect.width - 180. / 640. * 650.).abs() < 1e-9);
        assert!((rect.x + rect.width / 2. - 960.).abs() < 1e-9);
        assert_eq!(rect.y, 100.);
        let AssetProbe::Video(probe) = &mut p.assets[0].probe else {
            panic!()
        };
        probe.metadata.video.display_matrix = Some("invalid".into());
        assert!(diagram_rect(&p, &id).is_err());
    }
}
