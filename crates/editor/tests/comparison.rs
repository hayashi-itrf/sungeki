use sungeki_editor::{DifferenceKind, compare_projects};
use sungeki_project::*;

fn id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn fixture() -> Project {
    let mut p = Project::empty(id(1), "before".into());
    p.tracks.push(Track {
        id: id(2),
        name: "text".into(),
        visible: true,
        muted: false,
        kind: TrackKind::Visual,
    });
    for n in 3..5 {
        p.clips.push(Clip {
            id: id(n),
            track_id: id(2),
            scene_id: None,
            start_us: TimeUs(0),
            duration_us: TimeUs(100),
            transform: Transform {
                motion: None,
                x: 0.,
                y: 0.,
                width: 100.,
                height: 100.,
                opacity: 1.,
                fade: None,
                flip_x: None,
            },
            content: ClipContent::Text {
                outline: None,
                layout: None,
                text: "before".into(),
                font_size: 20.,
                color: "#000000".into(),
            },
        });
    }
    p
}
#[test]
fn compares_fields_by_identity_and_reports_render_order_separately() {
    let before = fixture();
    assert!(compare_projects(&before, &before).unwrap().is_empty());
    let mut after = before.clone();
    after.clips.reverse();
    after.clips[0].transform.x = 20.;
    let differences = compare_projects(&before, &after).unwrap();
    assert_eq!(differences.len(), 2);
    let field = differences
        .iter()
        .find(|d| d.kind == DifferenceKind::Changed)
        .unwrap();
    assert_eq!(field.entity_id, Some(id(4)));
    assert_eq!(field.field, "/transform/x");
    assert_eq!(field.before.as_ref().unwrap().json, "0.0");
    assert_eq!(field.after.as_ref().unwrap().json, "20.0");
    assert_eq!(
        differences
            .iter()
            .filter(|d| d.kind == DifferenceKind::Reordered)
            .count(),
        1
    );
}
#[test]
fn insertions_and_removals_do_not_claim_a_reorder() {
    let before = fixture();
    let mut after = before.clone();
    after.clips.remove(0);
    let mut inserted = before.clips[0].clone();
    inserted.id = id(5);
    after.clips.insert(0, inserted);
    let differences = compare_projects(&before, &after).unwrap();
    assert_eq!(differences.len(), 2);
    assert!(
        differences
            .iter()
            .any(|d| d.entity_id == Some(id(3)) && d.kind == DifferenceKind::Removed)
    );
    assert!(
        differences
            .iter()
            .any(|d| d.entity_id == Some(id(5)) && d.kind == DifferenceKind::Added)
    );
}
#[test]
fn long_multibyte_values_are_explicitly_truncated() {
    let before = fixture();
    let mut after = before.clone();
    if let ClipContent::Text { text, .. } = &mut after.clips[0].content {
        *text = "字幕".repeat(3000);
    }
    let differences = compare_projects(&before, &after).unwrap();
    assert_eq!(differences.len(), 1);
    assert_eq!(differences[0].field, "/content/text");
    let value = differences[0].after.as_ref().unwrap();
    assert!(value.truncated);
    assert_eq!(value.json.chars().count(), 4096);
    assert!(!differences[0].before.as_ref().unwrap().truncated);
}
#[test]
fn invalid_and_foreign_projects_are_rejected() {
    let before = fixture();
    let mut after = before.clone();
    after.id = id(9);
    assert!(compare_projects(&before, &after).is_err());
    after = before.clone();
    after.clips[0].track_id = id(9);
    assert!(compare_projects(&before, &after).is_err());
}

#[test]
fn scene_difference_and_membership_use_stable_entity_ids() {
    let mut before = fixture();
    before.scenes.push(Scene {
        fade: None,
        id: id(6),
        name: "Intro".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(200),
    });
    let mut after = before.clone();
    after.scenes[0].name = "Opening".into();
    after.clips[0].scene_id = Some(id(6));
    let differences = compare_projects(&before, &after).unwrap();
    assert_eq!(differences.len(), 2);
    assert!(
        differences
            .iter()
            .any(|d| d.collection == "scenes" && d.entity_id == Some(id(6)) && d.field == "/name")
    );
    assert!(
        differences.iter().any(|d| d.collection == "clips"
            && d.entity_id == Some(id(3))
            && d.field == "/scene_id")
    );
}

#[test]
fn credit_sized_text_is_readable_before_applying() {
    let before = fixture();
    let mut after = before.clone();
    let updated = "素材の作者と出典\n".repeat(30);
    if let ClipContent::Text { text, .. } = &mut after.clips[0].content {
        *text = updated.clone();
    }
    let differences = compare_projects(&before, &after).unwrap();
    let preview = differences[0].after.as_ref().unwrap();
    assert!(!preview.truncated);
    assert_eq!(
        serde_json::from_str::<String>(&preview.json).unwrap(),
        updated
    );
}
