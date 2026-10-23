use super::*;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Transform {
    Transform {
        x,
        y,
        width,
        height,
        opacity: 1.,
        flip_x: None,
        motion: None,
        fade: None,
    }
}

pub(super) fn configured() -> (Project, BuildRecipe, BuildScriptIds) {
    let (p, mut r, speech) = recipe_with_bgm();
    let mut settings = presentation::settings();
    settings.standing_offset_y = 0.;
    settings.slots = Some(BuildLayoutSlots {
        standing: [(r.roles[0].role.clone(), rect(-100., 800., 350., 650.))].into(),
        subtitle: Some(rect(160., 860., 1600., 200.)),
        diagram: Some(rect(0., 0., 1920., 1080.)),
        diagrams_behind_standings: true,
    });
    let mut ending = r
        .assets
        .iter()
        .find(|a| matches!(a.probe, AssetProbe::Image(_)))
        .unwrap()
        .clone();
    ending.id = id(980);
    ending.credit = "終了画面：専用画像の作者".into();
    settings.ending = Some(BuildEnding {
        duration_us: TimeUs(5_000_000),
        background_asset_id: Some(ending.id.clone()),
    });
    r.assets.push(ending);
    r.presentation = Some(settings);
    (p, r, speech)
}

#[test]
fn exact_slots_and_ending_keep_speech_timing_and_survive_undo_and_save() {
    for foreword in [false, true] {
        let (p, mut r, speech) = configured();
        let mut ids = layout_ids();
        if !foreword {
            r.foreword = None;
            ids.foreword = None;
        }
        r.validate().unwrap();
        let stage = synthesize(&p, &r, &speech);
        let mut history = History::new(stage.clone()).unwrap();
        history
            .apply(0, &plan_build_layout(&stage, &r, &speech, &ids).unwrap())
            .unwrap();
        let result = history.project().clone();
        let clip = |id: &String| result.clips.iter().find(|c| &c.id == id).unwrap();
        for id in &speech.speech_ids {
            let old = stage.clips.iter().find(|c| &c.id == id).unwrap();
            assert_eq!(clip(id).transform, rect(160., 860., 1600., 200.));
            assert_eq!(
                clip(id).start_us.0,
                old.start_us.0 + if foreword { 11_000_000 } else { 3_000_000 }
            );
            assert_eq!(clip(id).duration_us, old.duration_us);
            assert_eq!(clip(id).content, old.content);
        }
        for scene in &ids.scenes {
            for pair in &scene.standing {
                assert_eq!(clip(&pair[0]).transform, rect(-100., 800., 350., 650.));
                assert_ne!(clip(&pair[1]).transform, rect(-100., 800., 350., 650.));
            }
            for id in &scene.diagrams {
                assert_eq!(clip(id).transform, rect(0., 0., 1920., 1080.));
                let track_index =
                    |id: &String| result.tracks.iter().position(|t| &t.id == id).unwrap();
                assert!(
                    track_index(&clip(id).track_id)
                        < track_index(&clip(&scene.standing[0][0]).track_id)
                );
            }
        }
        let ending = result
            .scenes
            .iter()
            .find(|s| s.id == ids.credits_scene)
            .unwrap();
        assert_eq!(ending.duration_us, TimeUs(5_000_000));
        assert_eq!(clip(&ids.credits_clips[0]).duration_us, ending.duration_us);
        assert!(matches!(&clip(&ids.credits_clips[0]).content,
            ClipContent::Image {asset_id} if asset_id == &id(980)));
        assert!(matches!(&clip(&ids.credits_clips[1]).content,
            ClipContent::Text {text, ..} if text.contains("終了画面：専用画像の作者")));
        assert_eq!(
            clip(ids.bgm.as_ref().unwrap()).duration_us,
            result.duration().unwrap()
        );
        assert_eq!(
            result.duration().unwrap().0,
            stage.duration().unwrap().0 + if foreword { 16_000_000 } else { 8_000_000 }
        );
        let saved: Project = serde_json::from_slice(&serde_json::to_vec(&result).unwrap()).unwrap();
        assert_eq!(saved, result);
        history.undo(1).unwrap();
        assert_eq!(history.project(), &stage);
        history.redo(2).unwrap();
        assert_eq!(history.project(), &result);
    }
}

#[test]
fn slots_and_ending_reject_ambiguous_or_invalid_input_before_synthesis() {
    let (_, original, _) = configured();
    let reject = |change: fn(&mut BuildPresentation)| {
        let mut r = original.clone();
        change(r.presentation.as_mut().unwrap());
        assert!(r.validate().is_err());
    };
    reject(|p| {
        p.slots
            .as_mut()
            .unwrap()
            .standing
            .insert("unknown".into(), rect(0., 0., 1., 1.));
    });
    reject(|p| p.standing_offset_y = 1.);
    reject(|p| p.slots.as_mut().unwrap().subtitle.as_mut().unwrap().flip_x = Some(true));
    reject(|p| p.slots.as_mut().unwrap().diagram.as_mut().unwrap().width = 0.);
    reject(|p| p.slots.as_mut().unwrap().diagram.as_mut().unwrap().x = f64::NAN);
    reject(|p| p.ending.as_mut().unwrap().duration_us = TimeUs(0));
    reject(|p| p.ending.as_mut().unwrap().duration_us = TimeUs(60_000_001));
    reject(|p| p.ending.as_mut().unwrap().background_asset_id = Some(id(999)));
    reject(|p| p.ending.as_mut().unwrap().duration_us = TimeUs(500_000)); // fade no longer fits
    let mut r = original.clone();
    r.presentation
        .as_mut()
        .unwrap()
        .ending
        .as_mut()
        .unwrap()
        .background_asset_id = Some(r.bgm.as_ref().unwrap().asset_id.clone());
    assert!(r.validate().is_err());
    let mut r = original;
    r.scenes[0].diagrams[0].reaction = Some(BuildReaction {
        center_x: 960.,
        center_y: 400.,
        width: 500.,
        height: 300.,
        start_scale: 0.5,
        fade: VisualFade {
            in_us: TimeUs(0),
            out_us: TimeUs(0),
        },
    });
    assert!(r.validate().is_err());
}
