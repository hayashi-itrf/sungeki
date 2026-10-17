use super::*;

#[test]
fn portrait_narration_has_separate_diagram_standing_and_subtitle_regions() {
    let (p, mut r, speech) = recipe_with_bgm();
    r.preset_id = "narration-portrait-v1".into();
    r.roles.truncate(1);
    r.foreword.as_mut().unwrap().preset_id = "foreword-portrait-v1".into();
    for scene in &mut r.scenes {
        for line in &mut scene.lines {
            line.role = r.roles[0].role.clone();
            line.pose = None;
        }
    }
    // Landscape source with rotated EXIF must retain the oriented aspect ratio.
    let background_id = r.scenes[0].background_asset_id.clone();
    let background = r.assets.iter_mut().find(|a| a.id == background_id).unwrap();
    if let AssetProbe::Image(probe) = &mut background.probe {
        probe.width = 1920;
        probe.height = 1080;
        probe.orientation = 6;
    } else {
        panic!("image fixture required");
    }
    let stage = synthesize(&p, &r, &speech);
    assert_eq!((stage.canvas.width, stage.canvas.height), (1080, 1920));
    let ids = layout_ids();
    let commands = plan_build_layout(&stage, &r, &speech, &ids).unwrap();
    let full = apply_batch(&stage, &commands).unwrap();
    let find = |id: &str| full.clips.iter().find(|c| c.id == id).unwrap();
    for scene in &ids.scenes {
        let diagram = &find(&scene.diagrams[0]).transform;
        assert!(diagram.x >= 60. && diagram.x + diagram.width <= 1020.);
        assert!(diagram.y + diagram.height <= 740.);
        let standing = &find(&scene.standing[0][0]).transform;
        assert!(standing.y >= 760. && standing.y + standing.height <= 1440.);
        assert!(standing.x >= 0. && standing.x + standing.width <= 1080.);
        let backdrop = find(&scene.standing[0][1]);
        assert!(matches!(&backdrop.content, ClipContent::Shape { fill, .. } if fill == "#000000"));
        assert_eq!(backdrop.scene_id, find(&scene.standing[0][0]).scene_id);
        assert_eq!(backdrop.start_us, find(&scene.standing[0][0]).start_us);
        assert_eq!(
            backdrop.duration_us,
            find(&scene.standing[0][0]).duration_us
        );
        assert_eq!(backdrop.transform.opacity, 0.82);
        assert!(backdrop.transform.y >= standing.y + standing.height);
        let backdrop_track = full
            .tracks
            .iter()
            .position(|t| t.id == backdrop.track_id)
            .unwrap();
        let subtitle_track = full
            .tracks
            .iter()
            .position(|t| t.id == speech.track_id)
            .unwrap();
        assert!(backdrop_track < subtitle_track);
    }
    for id in &speech.speech_ids {
        let c = find(id);
        assert_eq!(c.transform.y, 1480.);
        assert!(c.transform.y + c.transform.height <= 1800.);
        assert!(c.start_us.0 >= 11_000_000);
    }
    for clip in &full.clips {
        if matches!(
            clip.content,
            ClipContent::Text { .. } | ClipContent::Shape { .. } | ClipContent::Blur { .. }
        ) {
            assert!(clip.transform.x >= 0. && clip.transform.x + clip.transform.width <= 1080.);
            assert!(clip.transform.y >= 0. && clip.transform.y + clip.transform.height <= 1920.);
        }
    }
    let background = &find(&ids.scenes[0].background).transform;
    let foreword = &find(&ids.foreword.as_ref().unwrap().clips[0]).transform;
    for image in [background, foreword] {
        assert!((image.width / image.height - 1080. / 1920.).abs() < 1e-10);
        assert!(image.x <= 0. && image.x + image.width >= 1080.);
        assert!(image.y <= 0. && image.y + image.height >= 1920.);
    }
    r.foreword.as_mut().unwrap().preset_id = "foreword-wide-v1".into();
    assert!(r.validate().is_err());
}
