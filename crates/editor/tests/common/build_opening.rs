use super::*;

fn range(start: u64, duration: u64) -> BuildOpeningRange {
    BuildOpeningRange {
        start_us: TimeUs(start),
        duration_us: TimeUs(duration),
    }
}
pub(super) fn fixture_opening() -> (Project, BuildRecipe, BuildScriptIds, BuildLayoutIds) {
    let (p, mut r, speech) = recipe_with_bgm();
    let mut ids = layout_ids();
    let psd: Project = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let asset = psd
        .assets
        .iter()
        .find(|a| matches!(a.probe, AssetProbe::Psd(_)))
        .unwrap();
    let ClipContent::Psd {
        pose,
        speech: Some(binding),
        ..
    } = &psd
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Psd { .. }))
        .unwrap()
        .content
    else {
        panic!()
    };
    r.assets.push(asset.clone());
    for role in &mut r.roles {
        let mut binding = binding.clone();
        binding.character_id = role.character.id.clone();
        role.standing = BuildStanding::Psd {
            asset_id: asset.id.clone(),
            pose: pose.clone(),
            speech: binding,
            flip_x: false,
        };
    }
    let (voice, _) = audio::result(89, 2000);
    let audio = BuildOpeningAudio {
        asset_id: voice.id.clone(),
        range: range(0, 2_000_000),
        playback: AudioPlayback {
            source_start_us: TimeUs(0),
            source_duration_us: TimeUs(2_000_000),
            looped: false,
            gain_db: 0.,
            volume_points: vec![],
            fade_in_us: TimeUs(0),
            fade_out_us: TimeUs(0),
            ducking: None,
        },
    };
    r.assets.push(voice);
    let rect = Transform {
        x: 100.,
        y: 600.,
        width: 350.,
        height: 500.,
        opacity: 1.,
        flip_x: None,
        motion: None,
        fade: None,
    };
    let opening = BuildOpening {
        duration_us: TimeUs(6_000_000),
        background_asset_id: r.scenes[0].background_asset_id.clone(),
        logo: Some(BuildOpeningImage {
            asset_id: r.scenes[0].diagrams[0].asset_id.clone(),
            range: range(0, 3_000_000),
            transform: rect.clone(),
            fade: Some(VisualFade {
                in_us: TimeUs(100_000),
                out_us: TimeUs(100_000),
            }),
        }),
        // Independent overlapping sources, both must survive track separation.
        audio: vec![
            audio.clone(),
            BuildOpeningAudio {
                range: range(1_000_000, 2_000_000),
                ..audio
            },
        ],
        performers: r
            .roles
            .iter()
            .map(|r| BuildOpeningPerformer {
                role: r.role.clone(),
                transform: rect.clone(),
                mouth_open: vec![range(100_000, 200_000), range(1_200_000, 1_800_000)],
            })
            .collect(),
        theme: BuildOpeningText {
            text: "今日のテーマ".into(),
            range: range(3_500_000, 2_500_000),
            transform: rect,
            font_size: 64.,
            color: "#ffffff".into(),
            outline: None,
            layout: SubtitleLayout {
                alignment: TextAlignment::Center,
                wrap: true,
            },
        },
    };
    let mut serial = 1000;
    let mut next = || {
        serial += 1;
        id(serial)
    };
    ids.opening = Some(BuildOpeningIds {
        logo: Some(next()),
        audio: opening.audio.iter().map(|_| next()).collect(),
        performers: opening
            .performers
            .iter()
            .map(|p| {
                p.intervals(opening.duration_us)
                    .iter()
                    .map(|_| next())
                    .collect()
            })
            .collect(),
    });
    let mut settings = presentation::settings();
    settings.opening = Some(opening);
    r.presentation = Some(settings);
    (p, r, speech, ids)
}

#[test]
fn opening_keeps_recorded_audio_mouth_cues_and_speech_timing_in_one_transaction() {
    for foreword in [true, false] {
        let (p, mut r, speech, mut ids) = fixture_opening();
        if !foreword {
            r.foreword = None;
            ids.foreword = None;
        }
        let stage = synthesize(&p, &r, &speech);
        let commands = plan_build_layout(&stage, &r, &speech, &ids).unwrap();
        let mut history = History::new(stage.clone()).unwrap();
        history.apply(0, &commands).unwrap();
        let full = history.project().clone();
        let lead = if foreword { 8_000_000 } else { 0 };
        let opening = r.presentation.as_ref().unwrap().opening.as_ref().unwrap();
        let extra = ids.opening.as_ref().unwrap();
        let find = |id: &str| full.clips.iter().find(|c| c.id == id).unwrap();
        for original in &stage.clips {
            let c = find(&original.id);
            assert_eq!(c.start_us.0, original.start_us.0 + lead + 6_000_000);
            assert_eq!(c.content, original.content);
        }
        assert_ne!(
            find(&extra.audio[0]).track_id,
            find(&extra.audio[1]).track_id
        );
        for (index, (voice, id)) in opening.audio.iter().zip(&extra.audio).enumerate() {
            let c = find(id);
            assert_eq!(c.start_us.0, lead + index as u64 * 1_000_000);
            assert_eq!(c.duration_us, voice.range.duration_us);
            assert!(matches!(c.content, ClipContent::Audio { .. }));
        }
        for (performer, fixed) in opening.performers.iter().zip(&extra.performers) {
            let role = r.roles.iter().find(|r| r.role == performer.role).unwrap();
            let BuildStanding::Psd { pose, speech, .. } = &role.standing else {
                panic!()
            };
            let mut end = lead;
            for ((range, open), id) in performer.intervals(opening.duration_us).iter().zip(fixed) {
                let c = find(id);
                assert_eq!(c.start_us.0, end);
                end += c.duration_us.0;
                assert_eq!(c.start_us.0, lead + range.start_us.0);
                let ClipContent::Psd {
                    pose: actual,
                    speech: binding,
                    ..
                } = &c.content
                else {
                    panic!()
                };
                assert_eq!(*actual, speech.pose(pose, *open));
                assert!(binding.is_none());
                assert_eq!(c.transform, performer.transform);
            }
            assert_eq!(end, lead + opening.duration_us.0);
        }
        assert_eq!(find(&ids.title_clips[1]).start_us.0, lead + 3_500_000);
        let saved: Project = serde_json::from_slice(&serde_json::to_vec(&full).unwrap()).unwrap();
        assert_eq!(saved, full);
        history.undo(1).unwrap();
        assert_eq!(history.project(), &stage);
        history.redo(2).unwrap();
        assert_eq!(history.project(), &full);
    }
}

#[test]
fn opening_rejects_untrusted_intervals_and_mismatched_fixed_ids() {
    let (p, r, speech, mut ids) = fixture_opening();
    for alter in [
        |o: &mut BuildOpening| o.duration_us = TimeUs(u64::MAX),
        |o: &mut BuildOpening| o.performers[0].mouth_open[0].start_us = TimeUs(u64::MAX),
        |o: &mut BuildOpening| o.performers[0].mouth_open[1].start_us = TimeUs(0),
        |o: &mut BuildOpening| o.performers[0].role = "unknown".into(),
        |o: &mut BuildOpening| o.audio[0].playback.looped = true,
        |o: &mut BuildOpening| o.audio[0].range.duration_us = TimeUs(3_000_000),
        |o: &mut BuildOpening| o.background_asset_id = o.audio[0].asset_id.clone(),
        |o: &mut BuildOpening| o.theme.transform.width = f64::NAN,
        |o: &mut BuildOpening| o.theme.range = range(5_000_000, 2_000_000),
    ] {
        let mut changed = r.clone();
        alter(
            changed
                .presentation
                .as_mut()
                .unwrap()
                .opening
                .as_mut()
                .unwrap(),
        );
        assert!(plan_build_script(&p, &changed, &speech).is_err());
    }
    let stage = synthesize(&p, &r, &speech);
    ids.opening.as_mut().unwrap().performers[0].pop();
    assert!(plan_build_layout(&stage, &r, &speech, &ids).is_err());
}
