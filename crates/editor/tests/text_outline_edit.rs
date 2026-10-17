use sungeki_editor::{Command, History};
use sungeki_project::*;

#[test]
fn outline_edit_preserves_text_and_survives_history_and_serialization() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scene-evaluation.json"
    ))
    .unwrap();
    let p: Project = serde_json::from_value(fixture["project"].clone()).unwrap();
    let id = p
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Text { .. }))
        .unwrap()
        .id
        .clone();
    let mut h = History::new(p.clone()).unwrap();
    let style = Some(SubtitleOutline {
        width: 3,
        color: "#123456".into(),
    });
    h.apply(
        0,
        &[Command::SetTextOutline {
            clip_id: id.clone(),
            outline: style.clone(),
        }],
    )
    .unwrap();
    let mut expected = p.clone();
    if let ClipContent::Text { outline: value, .. } = &mut expected
        .clips
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .content
    {
        *value = style;
    }
    assert_eq!(h.project(), &expected);
    let decoded: Project =
        serde_json::from_str(&serde_json::to_string(h.project()).unwrap()).unwrap();
    assert_eq!(decoded, expected);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &expected);
    h.apply(
        3,
        &[Command::SetTextOutline {
            clip_id: id.clone(),
            outline: None,
        }],
    )
    .unwrap();
    assert!(matches!(
        &h.project()
            .clips
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .content,
        ClipContent::Text { outline: None, .. }
    ));
    let before = h.project().clone();
    let wrong = p
        .clips
        .iter()
        .find(|c| !matches!(c.content, ClipContent::Text { .. }))
        .unwrap()
        .id
        .clone();
    assert!(
        h.apply(
            4,
            &[
                Command::RenameProject {
                    title: "must rollback".into()
                },
                Command::SetTextOutline {
                    clip_id: wrong,
                    outline: None
                }
            ]
        )
        .is_err()
    );
    assert_eq!(h.project(), &before);
}

#[test]
fn invalid_outline_rejects_whole_batch_and_direct_project_load() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scene-evaluation.json"
    ))
    .unwrap();
    let project: Project = serde_json::from_value(fixture["project"].clone()).unwrap();
    let id = project
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Text { .. }))
        .unwrap()
        .id
        .clone();
    for (width, color) in [(0, "#ffffff"), (33, "#ffffff"), (3, "bad-color")] {
        let value = Some(SubtitleOutline {
            width,
            color: color.into(),
        });
        let mut h = History::new(project.clone()).unwrap();
        assert!(
            h.apply(
                0,
                &[
                    Command::RenameProject {
                        title: "must rollback".into()
                    },
                    Command::SetTextOutline {
                        clip_id: id.clone(),
                        outline: value.clone()
                    }
                ]
            )
            .is_err()
        );
        assert_eq!(h.project(), &project);
        let mut invalid = project.clone();
        let ClipContent::Text { outline, .. } = &mut invalid
            .clips
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .content
        else {
            panic!()
        };
        *outline = value;
        assert!(validate(&invalid).is_err());
    }
}
