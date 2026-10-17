use sungeki_editor::{Command, EditAction, EditRequest, EditSession};
use sungeki_project::{AssetLocator, Project};

#[test]
fn next_image_receipts_upgrade_preserving_history_and_retry_identity() {
    let project: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/sprite-project.json")).unwrap();
    let asset = project
        .assets
        .iter()
        .find(|asset| matches!(asset.probe, sungeki_project::AssetProbe::Image(_)))
        .unwrap();
    let request = EditRequest {
        operation_id: "00000000-0000-4000-8000-000000000099".into(),
        expected_revision: 0,
        action: EditAction::Apply {
            commands: vec![Command::RelinkAsset {
                asset_id: asset.id.clone(),
                locator: AssetLocator::Absolute {
                    path: "/moved/image.png".into(),
                },
                probe: asset.probe.clone(),
            }],
        },
    };
    let mut session = EditSession::new(project.clone()).unwrap();
    let receipt = session.execute(request.clone()).unwrap();
    let current = session.snapshot().unwrap();
    let mut legacy: serde_json::Value = serde_json::from_slice(&current).unwrap();
    let record = legacy["receipts"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    let command = &mut record["request"]["action"]["commands"][0];
    command["probe"] = command["probe"]["info"].take();
    // This is recovery of our Sungeki journal, not acceptance of old API arguments.
    assert!(serde_json::from_value::<Command>(command.clone()).is_err());
    // Early Sungeki snapshots also stored untagged image probes in retained states.
    let history = legacy["history"].as_object_mut().unwrap();
    let untag = |project: &mut serde_json::Value| {
        for asset in project["assets"].as_array_mut().unwrap() {
            if asset["probe"]["type"] == "image" {
                asset["probe"] = asset["probe"]["info"].take();
            }
        }
    };
    untag(history.get_mut("project").unwrap());
    for key in ["undo", "redo"] {
        for entry in history.get_mut(key).unwrap().as_array_mut().unwrap() {
            untag(&mut entry["project"]);
        }
    }
    let mut restored = EditSession::restore(&serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(restored.snapshot().unwrap(), current);
    assert_eq!(restored.execute(request).unwrap(), receipt);
    restored
        .execute(EditRequest {
            operation_id: "00000000-0000-4000-8000-000000000098".into(),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: receipt.operation_id,
            },
        })
        .unwrap();
    assert_eq!(restored.history().project(), &project);
}
