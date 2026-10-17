use super::*;
#[test]
fn sprite_roles_cover_dialogue_and_portrait_with_bound_character_and_credits() {
    for portrait in [false, true] {
        let (p, mut r, ids) = recipe_with_bgm();
        let BuildStanding::Image { asset_id, .. } = &r.roles[0].standing else {
            panic!()
        };
        let asset_id = asset_id.clone();
        let sprites = SpriteSet {
            default_pose: "通常".into(),
            poses: vec![SpritePose {
                name: "通常".into(),
                closed_asset_id: asset_id.clone(),
                open_asset_id: Some(asset_id),
            }],
        };
        r.roles[0].standing = BuildStanding::Sprite {
            sprites: sprites.clone(),
            pose: None,
            flip_x: true,
        };
        r.roles[0].preset_id = "sprite-custom-v1".into();
        if portrait {
            r.preset_id = "narration-portrait-v1".into();
            r.roles.truncate(1);
            if let Some(foreword) = &mut r.foreword {
                foreword.preset_id = "foreword-portrait-v1".into();
            }
            let role = r.roles[0].role.clone();
            for scene in &mut r.scenes {
                for line in &mut scene.lines {
                    line.role = role.clone();
                }
            }
        }
        let stage = synthesize(&p, &r, &ids);
        let layout = layout_ids();
        let commands = plan_build_layout(&stage, &r, &ids, &layout).unwrap();
        let full = apply_batch(&stage, &commands).unwrap();
        let mut found = 0;
        for clip in &full.clips {
            if let ClipContent::Sprite {
                character_id,
                sprites: saved,
                ..
            } = &clip.content
            {
                found += 1;
                assert_eq!(saved, &sprites);
                assert_eq!(
                    character_id.as_deref(),
                    Some(r.roles[0].character.id.as_str())
                );
                assert_eq!(clip.transform.flip_x, Some(true));
                assert!(clip.transform.width > 0. && clip.transform.height > 0.);
                assert!(clip.scene_id.is_some());
            }
        }
        assert!(found > 0);
        let mut bad = r.clone();
        let BuildStanding::Sprite { pose, .. } = &mut bad.roles[0].standing else {
            panic!()
        };
        *pose = Some("missing".into());
        assert!(bad.validate().is_err());
    }
}

#[test]
fn line_sprite_pose_persists_through_other_speaker_and_resets_each_scene() {
    let (p, mut r, ids) = recipe_with_bgm();
    let BuildStanding::Image { asset_id, .. } = &r.roles[0].standing else {
        panic!()
    };
    let asset_id = asset_id.clone();
    r.roles[0].standing = BuildStanding::Sprite {
        sprites: SpriteSet {
            default_pose: "通常".into(),
            poses: ["通常", "笑顔"]
                .into_iter()
                .map(|name| SpritePose {
                    name: name.into(),
                    closed_asset_id: asset_id.clone(),
                    open_asset_id: None,
                })
                .collect(),
        },
        pose: None,
        flip_x: false,
    };
    r.roles[0].preset_id = "sprite-custom-v1".into();
    r.scenes[0].lines[0].sprite_pose = Some("笑顔".into());
    let stage = synthesize(&p, &r, &ids);
    let layout = layout_ids();
    let full = apply_batch(
        &stage,
        &plan_build_layout(&stage, &r, &ids, &layout).unwrap(),
    )
    .unwrap();
    for (scene_index, scene) in layout.scenes.iter().enumerate() {
        for line in &scene.standing {
            let clip = full.clips.iter().find(|c| c.id == line[0]).unwrap();
            let ClipContent::Sprite { pose, .. } = &clip.content else {
                panic!()
            };
            assert_eq!(
                pose.as_deref(),
                if scene_index == 0 {
                    Some("笑顔")
                } else {
                    None
                }
            );
        }
    }
    let mut bad = r.clone();
    bad.scenes[0].lines[0].sprite_pose = Some("unknown".into());
    assert!(bad.validate().is_err());
    bad = r.clone();
    bad.scenes[0].lines[1].sprite_pose = Some("笑顔".into());
    assert!(bad.validate().is_err()); // The second role still uses a static image.
    let encoded = serde_json::to_vec(&r).unwrap();
    assert_eq!(serde_json::from_slice::<BuildRecipe>(&encoded).unwrap(), r);
}
