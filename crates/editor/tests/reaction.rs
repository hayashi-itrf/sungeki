use sungeki_editor::{Command, History, apply_batch};
use sungeki_project::*;
fn fixture() -> Project {
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
    let hash = p.assets[0].content_hash.clone().unwrap();
    p.assets[0].probe = AssetProbe::Image(ImageProbe {
        format: ImageFormat::Png,
        width: 120,
        height: 120,
        orientation: 1,
        has_alpha: true,
        byte_length: 1234,
        sha256: hash,
    });
    p.clips[0].content = ClipContent::Image {
        asset_id: p.assets[0].id.clone(),
    };
    p.clips[0].transform.flip_x = Some(true);
    p
}
fn command(p: &Project) -> Command {
    Command::SetReactionMotion {
        clip_id: p.clips[0].id.clone(),
        center_x: 320.,
        center_y: 190.,
        width: 120.,
        height: 120.,
        start_scale: 0.6,
        fade: VisualFade {
            in_us: TimeUs(80000),
            out_us: TimeUs(150000),
        },
    }
}
#[test]
fn centered_reaction_is_repeatable_saved_and_undoable() {
    let p = fixture();
    let cmd = command(&p);
    let mut history = History::new(p.clone()).unwrap();
    history.apply(0, &[cmd.clone()]).unwrap();
    let result = history.project().clone();
    let clip = &result.clips[0];
    let before = &p.clips[0];
    assert_eq!(
        (
            &clip.content,
            &clip.scene_id,
            clip.start_us,
            clip.duration_us
        ),
        (
            &before.content,
            &before.scene_id,
            before.start_us,
            before.duration_us
        )
    );
    assert_eq!(clip.transform.flip_x, Some(true));
    for n in 0..=100 {
        let t = clip.transform.motion.as_ref().unwrap().sample(
            &clip.transform,
            TimeUs(clip.duration_us.0 * n / 100),
            clip.duration_us,
        );
        assert!((t.x + t.width / 2. - 320.).abs() < 1e-9);
        assert!((t.y + t.height / 2. - 190.).abs() < 1e-9);
    }
    assert_eq!(apply_batch(&result, &[cmd]).unwrap(), result);
    let saved: Project = serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
    assert_eq!(saved, result);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &p);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &result);
}
#[test]
fn invalid_or_non_image_reactions_reject_entire_batch() {
    let p = fixture();
    for bad in [0., -1., 1.1, f64::NAN] {
        let mut cmd = command(&p);
        if let Command::SetReactionMotion { start_scale, .. } = &mut cmd {
            *start_scale = bad;
        }
        assert!(apply_batch(&p, &[command(&p), cmd]).is_err());
    }
    let mut cmd = command(&p);
    if let Command::SetReactionMotion { fade, .. } = &mut cmd {
        fade.in_us = TimeUs(u64::MAX);
    }
    assert!(apply_batch(&p, &[cmd]).is_err());
    let mut other = p.clone();
    other.clips[0].content = ClipContent::Shape {
        shape: ShapeKind::Rectangle,
        fill: "#ffffff".into(),
    };
    assert!(apply_batch(&other, &[command(&other)]).is_err());
    for value in [f64::INFINITY, -100., 40000.] {
        let mut cmd = command(&p);
        if let Command::SetReactionMotion { width, .. } = &mut cmd {
            *width = value;
        }
        assert!(apply_batch(&p, &[cmd]).is_err());
    }
}
