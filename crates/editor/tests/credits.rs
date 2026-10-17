use sungeki_editor::*;
use sungeki_project::*;
fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn fixture() -> Project {
    let mut p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let mut text = p.clips[0].clone();
    text.id = id(30);
    text.content = ClipContent::Text {
        outline: None,
        layout: None,
        text: "手入力の内容".into(),
        font_size: 32.,
        color: "#111111".into(),
    };
    p.clips.push(text);
    p
}
fn refresh() -> Command {
    Command::RefreshSceneCredits {
        clip_id: id(30),
        scene_id: id(3),
    }
}
fn text(p: &Project) -> &str {
    let ClipContent::Text { text, .. } = &p.clips.last().unwrap().content else {
        panic!()
    };
    text
}
#[test]
fn scene_credit_update_is_explicit_undoable_and_preserves_layout() {
    let p = fixture();
    let mut h = History::new(p.clone()).unwrap();
    h.apply(0, &[refresh()]).unwrap();
    let next = h.project().clone();
    assert_eq!(text(&next), "Fixture only\ntest");
    assert_eq!(next.clips[2].transform, p.clips[2].transform);
    assert_eq!(next.clips[2].start_us, p.clips[2].start_us);
    assert_eq!(next.clips[2].duration_us, p.clips[2].duration_us);
    assert_eq!(next.clips[..2], p.clips[..2]);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &next);
}
#[test]
fn uses_only_scene_dependencies_deduplicates_and_refreshes_changed_metadata() {
    let mut p = fixture();
    p.clips[1].scene_id = Some(id(3));
    p.clips[1].duration_us = TimeUs(1_000_000);
    p.characters[0].credit = "voice attribution".into();
    p.assets[0].credit = "art attribution".into();
    let mut duplicate = p.clips[0].clone();
    duplicate.id = id(31);
    p.clips.insert(0, duplicate);
    let result = apply_batch(&p, &[refresh()]).unwrap();
    assert_eq!(text(&result), "art attribution\nvoice attribution");
    // Generated speech audio metadata is not an additional credit requirement.
    let result = apply_batch(
        &result,
        &[
            Command::SetAssetMetadata {
                asset_id: id(5),
                name: p.assets[0].name.clone(),
                source: None,
                credit: "updated art".into(),
            },
            refresh(),
        ],
    )
    .unwrap();
    assert_eq!(text(&result), "updated art\nvoice attribution");
}
#[test]
fn missing_credit_and_invalid_target_roll_back_whole_batch() {
    let p = fixture();
    for commands in [
        vec![
            Command::SetAssetMetadata {
                asset_id: id(5),
                name: "art".into(),
                source: None,
                credit: " ".into(),
            },
            refresh(),
        ],
        vec![Command::RefreshSceneCredits {
            clip_id: id(4),
            scene_id: id(3),
        }],
        vec![
            Command::SetClipScene {
                clip_id: id(30),
                scene_id: None,
            },
            refresh(),
        ],
        vec![Command::RefreshSceneCredits {
            clip_id: id(30),
            scene_id: id(99),
        }],
    ] {
        let mut h = History::new(p.clone()).unwrap();
        assert!(h.apply(0, &commands).is_err());
        assert_eq!(h.project(), &p);
    }
}

#[test]
fn multiline_credits_deduplicate_each_line_without_changing_metadata() {
    let mut p = fixture();
    p.assets[0].credit = " 共通の作者\r\n立ち絵の作者\n\n".into();
    p.characters[0].credit = "音声モデルの作者\n共通の作者\n音声モデルの作者".into();
    let mut h = History::new(p.clone()).unwrap();
    h.apply(0, &[refresh()]).unwrap();
    let result = h.project();
    let lines = text(result).lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 3);
    for expected in ["共通の作者", "立ち絵の作者", "音声モデルの作者"] {
        assert_eq!(lines.iter().filter(|line| **line == expected).count(), 1);
    }
    assert_eq!(result.assets, p.assets);
    assert_eq!(result.characters, p.characters);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
}
