use sungeki_editor::*;
use sungeki_project::*;
fn id(n: u8) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn fixture() -> (Project, ForewordTemplate) {
    let mut p = Project::empty(id(1), "会話".into());
    p.assets.push(Asset {
        id: id(2),
        name: "背景".into(),
        source: None,
        credit: "作者".into(),
        locator: AssetLocator::Absolute {
            path: "/missing.png".into(),
        },
        content_hash: Some("a".repeat(64)),
        probe: AssetProbe::Image(ImageProbe {
            format: ImageFormat::Png,
            width: 1920,
            height: 1080,
            orientation: 1,
            has_alpha: false,
            byte_length: 100,
            sha256: "a".repeat(64),
        }),
    });
    p.tracks.push(Track {
        id: id(3),
        name: "映像".into(),
        kind: TrackKind::Visual,
        visible: true,
        muted: false,
    });
    p.scenes.push(Scene {
        fade: None,
        id: id(4),
        name: "本文".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(1_000_000),
    });
    p.clips.push(Clip {
        id: id(5),
        track_id: id(3),
        scene_id: Some(id(4)),
        start_us: TimeUs(0),
        duration_us: TimeUs(1_000_000),
        transform: Transform {
            motion: None,
            x: 0.,
            y: 0.,
            width: 1920.,
            height: 1080.,
            opacity: 1.,
            fade: None,
            flip_x: None,
        },
        content: ClipContent::Image { asset_id: id(2) },
    });
    let mut outside = p.clips[0].clone();
    outside.id = id(6);
    outside.scene_id = None;
    outside.start_us = TimeUs(2_000_000);
    p.clips.push(outside);
    p.groups.push(Group {
        id: id(7),
        name: "保持".into(),
        clip_ids: vec![id(5)],
    });
    let t = ForewordTemplate {
        preset_id: "foreword-wide-v1".into(),
        background_asset_id: id(2),
        text: "前書き\n確認".into(),
        duration_us: TimeUs(8_000_000),
        scene_id: id(10),
        track_id: id(11),
        clip_ids: (12..16).map(id).collect(),
    };
    (p, t)
}
#[test]
fn preserves_contents_scene_group_and_relative_timing_with_undo() {
    let (p, t) = fixture();
    let mut h = History::new(p.clone()).unwrap();
    h.apply(
        0,
        &[Command::PrependForeword {
            template: t.clone(),
        }],
    )
    .unwrap();
    let result = h.project().clone();
    for (before, after) in p.clips.iter().zip(&result.clips) {
        let mut expected = before.clone();
        expected.start_us.0 += 8_000_000;
        assert_eq!(after, &expected);
    }
    assert_eq!(result.groups, p.groups);
    assert_eq!(result.scenes[0].start_us, TimeUs(8_000_000));
    assert_eq!(result.clips.len(), p.clips.len() + 4);
    assert_eq!(result.clips.last().unwrap().scene_id, Some(t.scene_id));
    let decoded: Project = serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
    assert_eq!(decoded, result);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &result);
}
#[test]
fn rejects_bad_inputs_and_late_collisions_without_partial_shift() {
    let (p, t) = fixture();
    for kind in 0..9 {
        let mut input = t.clone();
        let mut project = p.clone();
        match kind {
            0 => input.clip_ids[3] = id(5),
            1 => input.clip_ids.pop().map(|_| ()).unwrap(),
            2 => input.text = " ".into(),
            3 => input.text = "1\n2\n3\n4\n5".into(),
            4 => input.duration_us = TimeUs(1),
            5 => input.duration_us = TimeUs(MAX_TIME_US),
            6 => project.assets[0].credit.clear(),
            7 => project.canvas.width = 1080,
            _ => input.preset_id = "unknown".into(),
        }
        let mut history = History::new(project.clone()).unwrap();
        assert!(
            history
                .apply(0, &[Command::PrependForeword { template: input }])
                .is_err()
        );
        assert_eq!(history.project(), &project);
    }
}

#[test]
fn batch_reference_background_matches_explicit_foreword_without_replacing_text() {
    use std::collections::BTreeMap;
    let (original, mut t) = fixture();
    let mut base = original.clone();
    base.assets.clear();
    base.clips.clear();
    base.scenes.clear();
    base.groups.clear();
    let mut asset = original.assets[0].clone();
    let asset_id = asset.id.clone();
    asset.id = "$background".into();
    t.background_asset_id = "$background".into();
    t.text = "$background is literal".into();
    let commands = vec![
        Command::RegisterAsset { asset },
        Command::PrependForeword {
            template: t.clone(),
        },
    ];
    let plan = plan_batch_references(
        &base,
        &commands,
        &BTreeMap::from([("background".into(), asset_id.clone())]),
    )
    .unwrap();
    t.background_asset_id = asset_id;
    let expected = apply_batch(
        &base,
        &[
            Command::RegisterAsset {
                asset: original.assets[0].clone(),
            },
            Command::PrependForeword { template: t },
        ],
    )
    .unwrap();
    assert_eq!(apply_batch(&base, &plan.commands).unwrap(), expected);
}
