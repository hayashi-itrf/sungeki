use super::*;

pub(super) fn settings() -> BuildPresentation {
    BuildPresentation {
        fitting: None,
        structure: None,
        opening: None,
        slots: None,
        ending: None,
        standing_offset_y: 560.,
        credits: Some(BuildCreditsStyle {
            background: "#000000".into(),
            color: "#ffffff".into(),
            font_size: 56.,
            transform: Transform {
                x: 160.,
                y: 340.,
                width: 1600.,
                height: 400.,
                opacity: 1.,
                flip_x: None,
                motion: None,
                fade: None,
            },
            layout: SubtitleLayout {
                alignment: TextAlignment::Center,
                wrap: true,
            },
            fade: VisualFade {
                in_us: TimeUs(800_000),
                out_us: TimeUs(0),
            },
            omit_lines_containing: vec![],
        }),
        bgm_speech_envelope: Some(BuildSpeechEnvelope {
            speech_level: 0.25,
            ramp_us: TimeUs(500_000),
        }),
    }
}

#[test]
fn presentation_is_one_undoable_initial_layout_using_actual_speech_times() {
    for foreword in [true, false] {
        let (p, mut r, speech) = recipe_with_bgm();
        r.bgm.as_mut().unwrap().playback.fade_in_us = TimeUs(500_000);
        let mut ids = layout_ids();
        if !foreword {
            r.foreword = None;
            ids.foreword = None;
        }
        let stage = synthesize(&p, &r, &speech);
        let baseline = apply_batch(
            &stage,
            &plan_build_layout(&stage, &r, &speech, &ids).unwrap(),
        )
        .unwrap();
        r.presentation = Some(settings());
        let commands = plan_build_layout(&stage, &r, &speech, &ids).unwrap();
        let mut history = History::new(stage.clone()).unwrap();
        history.apply(0, &commands).unwrap();
        let result = history.project().clone();
        for id in ids.scenes.iter().flat_map(|s| s.standing.iter().flatten()) {
            let before = baseline.clips.iter().find(|c| &c.id == id).unwrap();
            let after = result.clips.iter().find(|c| &c.id == id).unwrap();
            assert_eq!(after.transform.y, before.transform.y + 560.);
            assert_eq!(before.content, after.content);
            assert_eq!(before.start_us, after.start_us);
        }
        let music = result
            .clips
            .iter()
            .find(|c| Some(&c.id) == ids.bgm.as_ref())
            .unwrap();
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
        let last = result
            .clips
            .iter()
            .filter(|c| matches!(c.content, ClipContent::Speech { .. }))
            .map(|c| c.start_us.0 + c.duration_us.0)
            .max()
            .unwrap();
        assert_eq!(music.start_us.0, 0);
        assert_eq!(playback.fade_in_us, TimeUs(500_000));
        assert_eq!(music.duration_us, result.duration().unwrap());
        assert_eq!(
            playback
                .volume_points
                .iter()
                .map(|p| (p.at_us.0, p.level))
                .collect::<Vec<_>>(),
            vec![
                (0, 1.),
                (first - 500_000, 1.),
                (first, 0.25),
                (last, 0.25),
                (last + 500_000, 1.),
                (music.duration_us.0, 1.)
            ]
        );
        let credits = result
            .clips
            .iter()
            .find(|c| c.id == ids.credits_clips[1])
            .unwrap();
        assert_eq!(credits.transform.y, 340.);
        assert!(
            matches!(&credits.content, ClipContent::Text { font_size, color, .. }
            if *font_size == 56. && color == "#ffffff")
        );
        assert_eq!(result.assets, baseline.assets);
        assert_eq!(result.duration(), baseline.duration());
        let saved: Project = serde_json::from_slice(&serde_json::to_vec(&result).unwrap()).unwrap();
        assert_eq!(saved, result);
        history.undo(1).unwrap();
        assert_eq!(history.project(), &stage);
        history.redo(2).unwrap();
        assert_eq!(history.project(), &result);
    }
}

#[test]
fn presentation_rejects_ambiguous_audio_and_invalid_display_settings() {
    let (p, mut r, speech) = recipe_with_bgm();
    r.presentation = Some(settings());
    r.validate().unwrap();
    r.bgm
        .as_mut()
        .unwrap()
        .playback
        .volume_points
        .push(VolumePoint {
            at_us: TimeUs(0),
            level: 1.,
        });
    assert!(r.validate().is_err());
    r.bgm.as_mut().unwrap().playback.volume_points.clear();
    r.presentation
        .as_mut()
        .unwrap()
        .credits
        .as_mut()
        .unwrap()
        .omit_lines_containing = vec!["".into()];
    assert!(r.validate().is_err());
    r.presentation = Some(settings());
    r.presentation
        .as_mut()
        .unwrap()
        .credits
        .as_mut()
        .unwrap()
        .fade
        .in_us = TimeUs(4_000_000);
    assert!(r.validate().is_err());
    r.presentation = Some(settings());
    r.presentation
        .as_mut()
        .unwrap()
        .credits
        .as_mut()
        .unwrap()
        .omit_lines_containing = r
        .assets
        .iter()
        .map(|a| a.credit.clone())
        .chain(r.roles.iter().map(|r| r.character.credit.clone()))
        .collect();
    let stage = synthesize(&p, &r, &speech);
    assert!(plan_build_layout(&stage, &r, &speech, &layout_ids()).is_err());
}
