use sungeki_editor::{Command, History};
use sungeki_project::{AssetProbe, Project};
#[test]
fn vrm_registration_roundtrip_undo_and_invalid_metadata_are_atomic() {
    let fixture: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/vrm-assets-project.json"
    ))
    .unwrap();
    sungeki_project::validate(&fixture).unwrap();
    let empty = Project::empty(fixture.id.clone(), "VRM".into());
    let mut history = History::new(empty.clone()).unwrap();
    history
        .apply(
            0,
            &fixture
                .assets
                .iter()
                .cloned()
                .map(|asset| Command::RegisterAsset { asset })
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let registered = history.project().clone();
    assert_eq!(
        serde_json::from_slice::<Project>(&serde_json::to_vec(&registered).unwrap()).unwrap(),
        registered
    );
    let mut bad = fixture.assets[0].clone();
    bad.id = "00000000-0000-4000-8000-000000000009".into();
    let AssetProbe::Vrm(p) = &mut bad.probe else {
        panic!()
    };
    p.inspection.expressions.push("aa".into());
    assert!(
        history
            .apply(1, &[Command::RegisterAsset { asset: bad }])
            .is_err()
    );
    assert_eq!(history.project(), &registered);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &empty);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &registered);
}
