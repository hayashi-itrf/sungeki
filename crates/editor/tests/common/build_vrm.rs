use super::*;
#[test]
fn vrm_layout_preserves_scene_clock_binding_and_geometry() {
    for portrait in [false, true] {
        let (p, mut r, ids) = recipe_with_bgm();
        let mut fixture: Project = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/vrm-clip-project.json"
        ))
        .unwrap();
        let ClipContent::Vrm { mut standing } = fixture.clips[0].content.clone() else {
            panic!()
        };
        standing.character_id = Some(r.roles[0].character.id.clone());
        for asset in &mut fixture.assets {
            let previous = asset.id.clone();
            asset.id = uuid::Uuid::new_v4().to_string();
            if standing.model_asset_id == previous {
                standing.model_asset_id = asset.id.clone();
            }
            if standing.motion_asset_id == previous {
                standing.motion_asset_id = asset.id.clone();
            }
        }
        r.assets.extend(fixture.assets);
        r.roles[0].standing = BuildStanding::Vrm {
            standing: standing.clone(),
            flip_x: true,
        };
        r.roles[0].preset_id = "vrm-custom-v1".into();
        if portrait {
            r.preset_id = "narration-portrait-v1".into();
            r.roles.truncate(1);
            if let Some(f) = &mut r.foreword {
                f.preset_id = "foreword-portrait-v1".into();
            }
            let role = r.roles[0].role.clone();
            for scene in &mut r.scenes {
                for line in &mut scene.lines {
                    line.role = role.clone();
                }
            }
        }
        r.validate().unwrap();
        assert_eq!(
            serde_json::from_slice::<BuildRecipe>(&serde_json::to_vec(&r).unwrap()).unwrap(),
            r
        );
        let stage = synthesize(&p, &r, &ids);
        let layout = layout_ids();
        let full = apply_batch(
            &stage,
            &plan_build_layout(&stage, &r, &ids, &layout).unwrap(),
        )
        .unwrap();
        let mut found = 0;
        for clip in &full.clips {
            if let ClipContent::Vrm { standing: saved } = &clip.content {
                let scene = full
                    .scenes
                    .iter()
                    .find(|s| Some(&s.id) == clip.scene_id.as_ref())
                    .unwrap();
                assert_eq!(
                    saved.source_start_us.0,
                    standing.source_start_us.0 + clip.start_us.0 - scene.start_us.0
                );
                assert_eq!(saved.character_id, standing.character_id);
                assert_eq!(clip.transform.flip_x, Some(true));
                assert_eq!(
                    (clip.transform.width, clip.transform.height),
                    if portrait { (650., 680.) } else { (350., 650.) }
                );
                found += 1;
            }
        }
        assert_eq!(found, 4);
        let mut bad = r.clone();
        let BuildStanding::Vrm { standing, .. } = &mut bad.roles[0].standing else {
            panic!()
        };
        standing.character_id = None;
        assert!(bad.validate().is_err());
        let mut bad = r.clone();
        let model = bad
            .assets
            .iter_mut()
            .find(|a| matches!(&a.probe, AssetProbe::Vrm(p) if p.inspection.kind == VrmKind::Model))
            .unwrap();
        model.credit.clear();
        assert!(bad.validate().is_err());
    }
}
