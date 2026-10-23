use super::*;
fn contain(x: f64, y: f64) -> BuildVisualFit {
    BuildVisualFit {
        mode: BuildFitMode::Contain,
        anchor_x: x,
        anchor_y: y,
    }
}

#[test]
fn differently_shaped_diagrams_fit_one_frame_before_save_and_keep_timing() {
    let (p, mut r, speech) = slots::configured();
    let first = r.scenes[0].diagrams[0].asset_id.clone();
    let asset = r.assets.iter_mut().find(|a| a.id == first).unwrap();
    let AssetProbe::Image(probe) = &mut asset.probe else {
        panic!()
    };
    probe.width = 400;
    probe.height = 100;
    probe.orientation = 1;
    let mut second = asset.clone();
    second.id = id(997);
    let AssetProbe::Image(probe) = &mut second.probe else {
        panic!()
    };
    probe.orientation = 6; // oriented 100 × 400, using the same encoded dimensions
    r.scenes[1].diagrams[0].asset_id = second.id.clone();
    r.assets.push(second);
    let presentation = r.presentation.as_mut().unwrap();
    presentation.fitting = Some(BuildVisualFits {
        diagram: Some(contain(1., 1.)),
        ..Default::default()
    });
    let frame = presentation
        .slots
        .as_mut()
        .unwrap()
        .diagram
        .as_mut()
        .unwrap();
    frame.x = 100.;
    frame.y = 50.;
    frame.width = 800.;
    frame.height = 600.;
    let stage = synthesize(&p, &r, &speech);
    let ids = layout_ids();
    let mut history = History::new(stage.clone()).unwrap();
    history
        .apply(0, &plan_build_layout(&stage, &r, &speech, &ids).unwrap())
        .unwrap();
    let result = history.project().clone();
    let first = result
        .clips
        .iter()
        .find(|c| c.id == ids.scenes[0].diagrams[0])
        .unwrap();
    let second = result
        .clips
        .iter()
        .find(|c| c.id == ids.scenes[1].diagrams[0])
        .unwrap();
    assert_eq!(
        (
            first.transform.x,
            first.transform.y,
            first.transform.width,
            first.transform.height
        ),
        (100., 450., 800., 200.)
    );
    assert_eq!(
        (
            second.transform.x,
            second.transform.y,
            second.transform.width,
            second.transform.height
        ),
        (750., 50., 150., 600.)
    );
    let saved: Project = serde_json::from_slice(&serde_json::to_vec(&result).unwrap()).unwrap();
    assert_eq!(result, saved);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &stage);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &result);
    let mut exact = r.clone();
    exact.presentation.as_mut().unwrap().fitting = None;
    let baseline = apply_batch(
        &stage,
        &plan_build_layout(&stage, &exact, &speech, &ids).unwrap(),
    )
    .unwrap();
    for clip in &result.clips {
        let old = baseline.clips.iter().find(|c| c.id == clip.id).unwrap();
        assert_eq!(clip.start_us, old.start_us);
        assert_eq!(clip.duration_us, old.duration_us);
        assert_eq!(clip.content, old.content);
    }
}

#[test]
fn opening_and_body_psd_preserve_native_ratio_and_explicit_bottom_anchor() {
    let (p, mut r, speech, ids) = opening::fixture_opening();
    let role = r.roles[0].role.clone();
    let opening = r.presentation.as_ref().unwrap().opening.as_ref().unwrap();
    let frame = opening.performers[0].transform.clone();
    let presentation = r.presentation.as_mut().unwrap();
    presentation.standing_offset_y = 0.;
    presentation.slots = Some(BuildLayoutSlots {
        standing: [(role.clone(), frame.clone())].into(),
        subtitle: None,
        diagram: None,
        diagrams_behind_standings: false,
    });
    presentation.fitting = Some(BuildVisualFits {
        standing: [(role.clone(), contain(0., 1.))].into(),
        opening_standing: [(role, contain(0., 1.))].into(),
        opening_logo: Some(contain(0.5, 0.5)),
        diagram: None,
    });
    let stage = synthesize(&p, &r, &speech);
    let result = apply_batch(
        &stage,
        &plan_build_layout(&stage, &r, &speech, &ids).unwrap(),
    )
    .unwrap();
    for id in ids.opening.as_ref().unwrap().performers[0].iter().chain(
        ids.scenes
            .iter()
            .flat_map(|s| s.standing.iter().map(|pair| &pair[0])),
    ) {
        let c = result.clips.iter().find(|c| &c.id == id).unwrap();
        assert!((c.transform.y + c.transform.height - frame.y - frame.height).abs() < 1e-8);
        assert!(c.transform.width <= frame.width && c.transform.height <= frame.height);
        let ClipContent::Psd { asset_id, .. } = &c.content else {
            panic!()
        };
        let AssetProbe::Psd(probe) = &result
            .assets
            .iter()
            .find(|a| &a.id == asset_id)
            .unwrap()
            .probe
        else {
            panic!()
        };
        assert!(
            (c.transform.width / c.transform.height - probe.width as f64 / probe.height as f64)
                .abs()
                < 1e-8
        );
    }
}

#[test]
fn invalid_or_missing_fit_targets_are_rejected_before_build() {
    let (_, mut r, _) = slots::configured();
    r.presentation.as_mut().unwrap().fitting = Some(BuildVisualFits {
        standing: [("unknown".into(), contain(0., 1.))].into(),
        ..Default::default()
    });
    assert!(r.validate().is_err());
    r.presentation.as_mut().unwrap().fitting = Some(BuildVisualFits {
        diagram: Some(contain(f64::NAN, 0.)),
        ..Default::default()
    });
    assert!(r.validate().is_err());
    r.presentation.as_mut().unwrap().fitting = Some(BuildVisualFits {
        opening_logo: Some(contain(0., 0.)),
        ..Default::default()
    });
    assert!(r.validate().is_err());
}
