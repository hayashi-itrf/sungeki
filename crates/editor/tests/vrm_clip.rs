use sungeki_editor::{Command, History};
use sungeki_project::*;
fn project() -> Project {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/vrm-clip-project.json"
    ))
    .unwrap()
}
#[test]
fn vrm_settings_roundtrip_scene_and_undo() {
    let p = project();
    validate(&p).unwrap();
    let mut h = History::new(p.clone()).unwrap();
    let ClipContent::Vrm { standing } = &p.clips[0].content else {
        panic!()
    };
    let mut next = standing.clone();
    next.expressions.insert("aa".into(), 1.0);
    h.apply(
        0,
        &[Command::SetVrm {
            clip_id: p.clips[0].id.clone(),
            standing: next,
        }],
    )
    .unwrap();
    let changed = h.project().clone();
    assert_eq!(changed.clips[0].scene_id, p.clips[0].scene_id);
    assert_eq!(
        serde_json::from_slice::<Project>(&serde_json::to_vec(&changed).unwrap()).unwrap(),
        changed
    );
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &changed);
}
#[test]
fn references_and_expression_changes_are_validated_atomically() {
    let p = project();
    let mut h = History::new(p.clone()).unwrap();
    for asset in &p.assets {
        assert!(
            h.apply(
                0,
                &[Command::RemoveAsset {
                    asset_id: asset.id.clone()
                }]
            )
            .is_err()
        );
        assert_eq!(h.project(), &p);
    }
    let ClipContent::Vrm { standing } = &p.clips[0].content else {
        panic!()
    };
    for case in 0..5 {
        let mut bad = standing.clone();
        match case {
            0 => bad.model_asset_id = bad.motion_asset_id.clone(),
            1 => {
                bad.expressions.insert("missing".into(), 1.0);
            }
            2 => {
                bad.expressions.insert("aa".into(), 1.1);
            }
            3 => bad.source_start_us = TimeUs(MAX_TIME_US),
            _ => bad.character_id = Some("missing".into()),
        }
        assert!(
            h.apply(
                0,
                &[Command::SetVrm {
                    clip_id: p.clips[0].id.clone(),
                    standing: bad
                }]
            )
            .is_err()
        );
        assert_eq!(h.project(), &p);
    }
}
#[test]
fn vrm_credits_include_model_and_motion() {
    let mut p = project();
    p.assets[0].credit = "モデル作者".into();
    p.assets[1].credit = "モーション作者".into();
    let mut text = p.clips[0].clone();
    text.id = "00000000-0000-4000-8000-000000000029".into();
    text.content = ClipContent::Text {
        outline: None,
        layout: None,
        text: "未更新".into(),
        font_size: 20.0,
        color: "#000000".into(),
    };
    p.clips.push(text.clone());
    let mut h = History::new(p.clone()).unwrap();
    h.apply(
        0,
        &[Command::RefreshSceneCredits {
            clip_id: text.id,
            scene_id: p.scenes[0].id.clone(),
        }],
    )
    .unwrap();
    let ClipContent::Text { text, .. } = &h.project().clips[1].content else {
        panic!()
    };
    assert!(text.contains("モデル作者") && text.contains("モーション作者"));
}
