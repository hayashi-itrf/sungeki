use sungeki_editor::{Command, History};
use sungeki_project::*;

#[test]
fn layout_edit_preserves_text_and_survives_history_and_serialization() {
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
    let layout = Some(SubtitleLayout {
        alignment: TextAlignment::Right,
        wrap: true,
    });
    h.apply(
        0,
        &[Command::SetTextLayout {
            clip_id: id.clone(),
            layout: layout.clone(),
        }],
    )
    .unwrap();
    let mut expected = p.clone();
    if let ClipContent::Text { layout: value, .. } = &mut expected
        .clips
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .content
    {
        *value = layout;
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
        &[Command::SetTextLayout {
            clip_id: id.clone(),
            layout: None,
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
        ClipContent::Text { layout: None, .. }
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
                Command::SetTextLayout {
                    clip_id: wrong,
                    layout: None
                }
            ]
        )
        .is_err()
    );
    assert_eq!(h.project(), &before);
}
