use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/speech.rs"]
mod setup;
use setup::*;
#[test]
fn outline_is_saved_and_undoable_and_invalid_styles_are_atomic() {
    let p = apply_batch(&Project::empty(id(1), "縁取り".into()), &commands()).unwrap();
    let mut character = character();
    character.subtitle_style.layout = Some(SubtitleLayout {
        alignment: TextAlignment::Center,
        wrap: true,
    });
    character.subtitle_style.outline = Some(SubtitleOutline {
        width: 3,
        color: "#000000".into(),
    });
    let mut h = History::new(p.clone()).unwrap();
    h.apply(
        0,
        &[Command::SetCharacter {
            character: character.clone(),
        }],
    )
    .unwrap();
    let next = h.project().clone();
    assert_eq!(next.clips, p.clips);
    assert_eq!(
        serde_json::from_str::<Project>(&serde_json::to_string(&next).unwrap()).unwrap(),
        next
    );
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &next);
    for (width, color) in [(0, "#000000"), (33, "#000000"), (3, "bad")] {
        character.subtitle_style.outline = Some(SubtitleOutline {
            width,
            color: color.into(),
        });
        assert!(
            h.apply(
                3,
                &[Command::SetCharacter {
                    character: character.clone()
                }]
            )
            .is_err()
        );
        assert_eq!(h.project(), &next);
    }
}
