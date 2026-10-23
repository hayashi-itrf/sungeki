use super::*;

#[test]
fn section_order_visibility_and_fixed_title_duration_preserve_members_and_credits() {
    use BuildBlock::*;
    for blocks in [
        vec![Opening, Foreword, Body, Credits],
        vec![Body, Credits],
        vec![Credits, Body, Opening, Foreword],
    ] {
        let (p, mut recipe, speech) = recipe_with_bgm();
        recipe.presentation = Some(presentation::settings());
        let stage = synthesize(&p, &recipe, &speech);
        let ids = layout_ids();
        let baseline = apply_batch(
            &stage,
            &plan_build_layout(&stage, &recipe, &speech, &ids).unwrap(),
        )
        .unwrap();
        recipe.presentation.as_mut().unwrap().structure = Some(BuildStructure {
            blocks: blocks.clone(),
            title_duration_us: Some(TimeUs(2_000_000)),
        });
        let mut history = History::new(stage.clone()).unwrap();
        history
            .apply(
                0,
                &plan_build_layout(&stage, &recipe, &speech, &ids).unwrap(),
            )
            .unwrap();
        let result = history.project().clone();
        let mut cursor = 0;
        for block in &blocks {
            let scenes = match block {
                Foreword => vec![ids.foreword.as_ref().unwrap().scene.clone()],
                Opening => vec![ids.title_scene.clone()],
                Body => speech.scene_ids.clone(),
                Credits => vec![ids.credits_scene.clone()],
            };
            for id in scenes {
                let scene = result.scenes.iter().find(|s| s.id == id).unwrap();
                assert_eq!(scene.start_us.0, cursor);
                if *block == Opening {
                    assert_eq!(scene.duration_us.0, 2_000_000);
                }
                cursor += scene.duration_us.0;
                if *block == Body {
                    let before_scene = baseline.scenes.iter().find(|s| s.id == id).unwrap();
                    for clip in result
                        .clips
                        .iter()
                        .filter(|c| c.scene_id.as_ref() == Some(&id))
                    {
                        let before = baseline.clips.iter().find(|c| c.id == clip.id).unwrap();
                        assert_eq!(
                            clip.start_us.0 - scene.start_us.0,
                            before.start_us.0 - before_scene.start_us.0
                        );
                        assert_eq!(clip.duration_us, before.duration_us);
                        assert_eq!(clip.content, before.content);
                    }
                }
            }
        }
        assert_eq!(result.duration().unwrap().0, cursor);
        let music = result
            .clips
            .iter()
            .find(|c| Some(&c.id) == ids.bgm.as_ref())
            .unwrap();
        assert_eq!(music.start_us, TimeUs(0));
        assert_eq!(music.duration_us.0, cursor);
        let ClipContent::Audio { playback, .. } = &music.content else {
            panic!()
        };
        let first = result
            .clips
            .iter()
            .filter(|c| matches!(c.content, ClipContent::Speech { .. }))
            .map(|c| c.start_us.0)
            .min()
            .unwrap();
        assert!(
            playback
                .volume_points
                .iter()
                .any(|p| p.at_us.0 == first && p.level == 0.25)
        );
        assert_eq!(result.assets, baseline.assets);
        assert!(result.clips.iter().any(|c| c.id == ids.credits_clips[1]));
        for (enabled, id) in [
            (
                blocks.contains(&Foreword),
                &ids.foreword.as_ref().unwrap().scene,
            ),
            (blocks.contains(&Opening), &ids.title_scene),
        ] {
            assert_eq!(result.scenes.iter().any(|s| &s.id == id), enabled);
            if !enabled {
                assert!(!result.clips.iter().any(|c| c.scene_id.as_ref() == Some(id)));
            }
        }
        history.undo(1).unwrap();
        assert_eq!(history.project(), &stage);
        history.redo(2).unwrap();
        assert_eq!(history.project(), &result);
        let restored: BuildRecipe =
            serde_json::from_slice(&serde_json::to_vec(&recipe).unwrap()).unwrap();
        assert_eq!(restored, recipe);
    }
}

#[test]
fn invalid_blocks_and_ambiguous_music_are_rejected_before_synthesis() {
    use BuildBlock::*;
    let (_, mut recipe, _) = recipe_with_bgm();
    recipe.presentation = Some(presentation::settings());
    for blocks in [vec![Body], vec![Credits], vec![Body, Body, Credits]] {
        recipe.presentation.as_mut().unwrap().structure = Some(BuildStructure {
            blocks,
            title_duration_us: None,
        });
        assert!(recipe.validate().is_err());
    }
    recipe.presentation.as_mut().unwrap().structure = Some(BuildStructure {
        blocks: vec![Body, Credits],
        title_duration_us: None,
    });
    recipe.validate().unwrap();
    recipe.bgm.as_mut().unwrap().playback.volume_points = vec![VolumePoint {
        at_us: TimeUs(0),
        level: 0.5,
    }];
    assert!(recipe.validate().is_err());
}
