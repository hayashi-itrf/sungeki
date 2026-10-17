use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/build_audio.rs"]
mod audio;
#[path = "common/build_recipe.rs"]
mod setup;
use setup::*;

fn layout_ids() -> BuildLayoutIds {
    BuildLayoutIds {
        tracks: std::array::from_fn(|n| id(500 + n as u32)),
        scenes: (0..2)
            .map(|n| BuildSceneVisualIds {
                background: id(510 + n * 10),
                standing: vec![
                    [id(511 + n * 10), id(512 + n * 10)],
                    [id(513 + n * 10), id(514 + n * 10)],
                ],
                diagrams: vec![id(515 + n * 10)],
            })
            .collect(),
        title_scene: id(540),
        title_clips: [id(541), id(542)],
        credits_scene: id(543),
        credits_clips: [id(544), id(545)],
        bgm: Some(id(550)),
        foreword: Some(BuildForewordIds {
            scene: id(560),
            track: id(561),
            clips: [id(562), id(563), id(564), id(565)],
        }),
    }
}
fn recipe_with_bgm() -> (Project, BuildRecipe, BuildScriptIds) {
    let (p, mut r, ids) = fixture();
    let (asset, _) = audio::result(88, 1000);
    r.bgm = Some(BuildBgm {
        volume_point_reference_duration_us: None,
        asset_id: asset.id.clone(),
        playback: AudioPlayback {
            source_start_us: TimeUs(0),
            source_duration_us: TimeUs(1_000_000),
            looped: true,
            gain_db: -20.,
            volume_points: vec![],
            fade_in_us: TimeUs(1_000_000),
            fade_out_us: TimeUs(1_000_000),
            ducking: None,
        },
    });
    r.assets.push(asset);
    (p, r, ids)
}
fn synthesize(p: &Project, r: &BuildRecipe, ids: &BuildScriptIds) -> Project {
    let mut p = apply_batch(p, &plan_build_script(p, r, ids).unwrap()).unwrap();
    for (index, clip_id) in ids.speech_ids.iter().enumerate() {
        let (asset, audio) = audio::result(index as u32, 2000);
        p = apply_batch(
            &p,
            &plan_build_speech_application(&p, r, ids, clip_id, asset, audio).unwrap(),
        )
        .unwrap();
    }
    p
}
#[test]
fn full_layout_preserves_scenes_and_covers_diagrams_bgm_foreword_and_credits() {
    let (p, r, speech) = recipe_with_bgm();
    let stage = synthesize(&p, &r, &speech);
    let ids = layout_ids();
    let commands = plan_build_layout(&stage, &r, &speech, &ids).unwrap();
    let mut h = History::new(stage.clone()).unwrap();
    h.apply(0, &commands).unwrap();
    let full = h.project().clone();
    let find = |id: &str| full.clips.iter().find(|c| c.id == id).unwrap();
    let shift = 11_000_000;
    for original in &stage.clips {
        let c = find(&original.id);
        assert_eq!(c.start_us.0, original.start_us.0 + shift);
        assert_eq!(c.content, original.content);
        assert_eq!(c.scene_id, original.scene_id);
    }
    for (n, sid) in ids.scenes.iter().enumerate() {
        let scene = full
            .scenes
            .iter()
            .find(|s| s.id == speech.scene_ids[n])
            .unwrap();
        assert_eq!(find(&sid.background).start_us, scene.start_us);
        assert_eq!(find(&sid.background).duration_us, scene.duration_us);
        let diagram = find(&sid.diagrams[0]);
        assert_eq!(diagram.start_us, find(&speech.speech_ids[n * 2]).start_us);
        assert_eq!(diagram.duration_us, scene.duration_us);
        assert!(diagram.transform.x >= 390. && diagram.transform.width <= 1140.);
        for role in 0..2 {
            let first = find(&sid.standing[0][role]);
            let last = find(&sid.standing[1][role]);
            assert_eq!(first.start_us.0 + first.duration_us.0, last.start_us.0);
            assert_eq!(
                last.start_us.0 + last.duration_us.0,
                scene.start_us.0 + scene.duration_us.0
            );
        }
    }
    let bgm = find(ids.bgm.as_ref().unwrap());
    assert_eq!(bgm.start_us, TimeUs(8_000_000));
    assert_eq!(bgm.duration_us.0, stage.duration().unwrap().0 + 6_500_000);
    assert!(
        matches!(&bgm.content, ClipContent::Audio { playback,.. } if *playback == r.bgm.as_ref().unwrap().playback)
    );
    let ClipContent::Text { text, .. } = &find(&ids.credits_clips[1]).content else {
        panic!()
    };
    for role in &r.roles {
        assert!(text.contains(&role.character.credit));
    }
    for asset in &r.assets {
        assert!(text.contains(&asset.credit));
    }
    assert_eq!(
        full.duration().unwrap().0,
        stage.duration().unwrap().0 + 14_500_000
    );
    assert!(plan_build_layout(&full, &r, &speech, &ids).is_err());
    h.undo(1).unwrap();
    assert_eq!(h.project(), &stage);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &full);
}
#[test]
fn rejects_missing_speech_bad_ids_and_bgm_that_exceeds_actual_length() {
    let (p, mut r, speech) = recipe_with_bgm();
    let stage = apply_batch(&p, &plan_build_script(&p, &r, &speech).unwrap()).unwrap();
    assert!(plan_build_layout(&stage, &r, &speech, &layout_ids()).is_err());
    let stage = synthesize(&p, &r, &speech);
    let mut ids = layout_ids();
    ids.scenes[0].standing.pop();
    assert!(plan_build_layout(&stage, &r, &speech, &ids).is_err());
    let mut ids = layout_ids();
    ids.title_clips[0] = speech.speech_ids[0].clone();
    assert!(plan_build_layout(&stage, &r, &speech, &ids).is_err());
    r.bgm.as_mut().unwrap().playback.fade_out_us = TimeUs(100_000_000);
    let stage = synthesize(&p, &r, &speech);
    assert!(plan_build_layout(&stage, &r, &speech, &layout_ids()).is_err());
}

#[test]
fn psd_pose_and_flip_persist_through_other_speaker_and_reset_at_scene_boundary() {
    let (p, mut r, speech) = fixture();
    let fixture: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let mut asset = fixture
        .assets
        .iter()
        .find(|a| matches!(a.probe, AssetProbe::Psd(_)))
        .unwrap()
        .clone();
    asset.id = id(900);
    let ClipContent::Psd {
        pose,
        speech: Some(binding),
        ..
    } = &fixture
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Psd { .. }))
        .unwrap()
        .content
    else {
        panic!()
    };
    let mut binding = binding.clone();
    binding.character_id = r.roles[0].character.id.clone();
    r.roles[0].standing = BuildStanding::Psd {
        asset_id: asset.id.clone(),
        pose: pose.clone(),
        speech: binding.clone(),
        flip_x: true,
    };
    r.assets.push(asset);
    let mut changed = pose.clone();
    changed.selections[0].option = binding.open_option.clone();
    r.scenes[0].lines[0].pose = Some(changed.clone());
    let stage = synthesize(&p, &r, &speech);
    let mut ids = layout_ids();
    ids.bgm = None;
    let full = apply_batch(
        &stage,
        &plan_build_layout(&stage, &r, &speech, &ids).unwrap(),
    )
    .unwrap();
    for (scene_index, scene) in ids.scenes.iter().enumerate() {
        for pair in &scene.standing {
            let clip = full.clips.iter().find(|c| c.id == pair[0]).unwrap();
            assert_eq!(clip.transform.flip_x, Some(true));
            assert!(
                matches!(&clip.content, ClipContent::Psd { pose: actual, speech: Some(actual_binding),.. }
                if *actual_binding == binding && *actual == if scene_index == 0 { changed.clone() } else { pose.clone() })
            );
        }
    }
    r.foreword = None;
    ids.foreword = None;
    let stage = synthesize(&p, &r, &speech);
    let full = apply_batch(
        &stage,
        &plan_build_layout(&stage, &r, &speech, &ids).unwrap(),
    )
    .unwrap();
    assert_eq!(
        full.duration().unwrap().0,
        stage.duration().unwrap().0 + 6_500_000
    );
}

#[test]
fn narration_has_one_speaker_and_preserves_diagrams_music_and_scene_ownership() {
    let (p, mut r, speech) = recipe_with_bgm();
    r.preset_id = "narration-wide-v1".into();
    r.roles.truncate(1);
    for scene in &mut r.scenes {
        for line in &mut scene.lines {
            line.role = r.roles[0].role.clone();
            line.pose = None;
        }
    }
    let stage = synthesize(&p, &r, &speech);
    let ids = layout_ids();
    let commands = plan_build_layout(&stage, &r, &speech, &ids).unwrap();
    let full = apply_batch(&stage, &commands).unwrap();
    assert_eq!(full.characters.len(), 1);
    for scene in &ids.scenes {
        for pair in &scene.standing {
            let standing = full.clips.iter().find(|c| c.id == pair[0]).unwrap();
            assert!(standing.scene_id.is_some());
            assert!(standing.transform.x < 390.);
            assert!(!full.clips.iter().any(|c| c.id == pair[1]));
        }
        assert!(full.clips.iter().any(|c| c.id == scene.diagrams[0]));
    }
    assert!(full.clips.iter().any(|c| Some(&c.id) == ids.bgm.as_ref()));
    let mut h = History::new(stage.clone()).unwrap();
    h.apply(0, &commands).unwrap();
    h.undo(1).unwrap();
    assert_eq!(h.project(), &stage);
    let saved: Project = serde_json::from_slice(&serde_json::to_vec(&full).unwrap()).unwrap();
    assert_eq!(saved, full);
    r.preset_id = "dialogue-wide-v1".into();
    assert!(r.validate().is_err());
    r.preset_id = "narration-wide-v1".into();
    r.roles.push(r.roles[0].clone());
    assert!(r.validate().is_err());
}

#[path = "common/build_portrait_cases.rs"]
mod portrait_cases;

#[path = "common/build_video_cases.rs"]
mod video_cases;

#[path = "common/build_sprite.rs"]
mod sprite_cases;

#[path = "common/build_vrm.rs"]
mod vrm_cases;

#[path = "common/build_reaction.rs"]
mod reaction;

#[path = "common/build_layout_scene_fade.rs"]
mod scene_fade;

#[path = "common/build_layout_motion.rs"]
mod motion;

#[path = "common/build_bgm_points.rs"]
mod bgm_points;

#[path = "common/build_facing.rs"]
mod facing;
