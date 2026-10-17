use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/build_recipe.rs"]
mod setup;
use setup::*;

#[path = "common/build_audio.rs"]
mod audio;
use audio::result;

#[test]
fn actual_lengths_ripple_across_scenes_atomically_and_restore_with_audio() {
    let (p, mut recipe, ids) = fixture();
    recipe.scenes[0].gap_us = TimeUs(375_000);
    recipe.scenes[1].gap_us = TimeUs(0);
    let stage = apply_batch(&p, &plan_build_script(&p, &recipe, &ids).unwrap()).unwrap();
    let mut h = History::new(stage.clone()).unwrap();
    let lengths = [3000, 500, 7000, 250];
    for (index, ms) in lengths.into_iter().enumerate() {
        let before = h.project().clone();
        let (asset, audio) = result(index as u32, ms);
        let commands = plan_build_speech_application(
            h.project(),
            &recipe,
            &ids,
            &ids.speech_ids[index],
            asset,
            audio,
        )
        .unwrap();
        let rev = h.revision();
        h.apply(rev, &commands).unwrap();
        let after = h.project().clone();
        let mut cursor = 0;
        for (n, clip) in after.clips.iter().enumerate() {
            let expected_duration = if n <= index {
                lengths[n] * 1000
            } else {
                1_000_000
            };
            assert_eq!(clip.start_us.0, cursor);
            assert_eq!(clip.duration_us.0, expected_duration);
            assert_eq!(clip.scene_id, Some(ids.scene_ids[n / 2].clone()));
            cursor += expected_duration;
            if n % 2 == 0 {
                cursor += recipe.scenes[n / 2].gap_us.0;
            }
        }
        assert_eq!(after.duration(), Some(TimeUs(cursor)));
        assert_eq!(
            after.scenes[1].start_us.0,
            after.scenes[0].start_us.0 + after.scenes[0].duration_us.0
        );
        let restored: Project =
            serde_json::from_slice(&serde_json::to_vec(&after).unwrap()).unwrap();
        assert_eq!(restored, after);
        h.undo(rev + 1).unwrap();
        assert_eq!(h.project(), &before);
        h.redo(rev + 2).unwrap();
        assert_eq!(h.project(), &after);
    }
    let (asset, audio) = result(5, 2000);
    assert!(
        plan_build_speech_application(h.project(), &recipe, &ids, &ids.speech_ids[3], asset, audio)
            .is_err()
    );
}

#[test]
fn refuses_out_of_order_or_edited_staging_and_invalid_artifact() {
    let (p, recipe, ids) = fixture();
    let stage = apply_batch(&p, &plan_build_script(&p, &recipe, &ids).unwrap()).unwrap();
    for mode in 0..8 {
        let mut changed = stage.clone();
        let (mut asset, mut audio) = result(0, 3000);
        let mut target = ids.speech_ids[0].clone();
        match mode {
            0 => target = ids.speech_ids[1].clone(),
            1 => changed.title = "ユーザーの編集".into(),
            2 => changed.clips[0].scene_id = None,
            3 => changed.clips[0].transform.x += 1.0,
            4 => changed.scenes[0].name = "別名".into(),
            5 => audio.asset_id = id(999),
            6 => asset.content_hash = Some("c".repeat(64)),
            _ => {
                (asset, audio) = result(0, 0);
            }
        }
        let before = changed.clone();
        assert!(
            plan_build_speech_application(&changed, &recipe, &ids, &target, asset, audio).is_err(),
            "mode {mode}"
        );
        assert_eq!(changed, before);
    }
}
