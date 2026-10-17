use sungeki_editor::{Command, EditAction, EditRequest, EditSession};
use sungeki_project::*;
fn project() -> Project {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/vrm-clip-project.json"
    ))
    .unwrap()
}
#[test]
fn pose_is_validated_saved_and_undoable() {
    let p = project();
    let id = p.clips[0].id.clone();
    let ClipContent::Vrm { standing } = &p.clips[0].content else {
        panic!()
    };
    let mut pose = standing.clone();
    pose.pose = Some([("leftUpperArm".into(), [0., 0., 60.])].into());
    let mut session = EditSession::new(p.clone()).unwrap();
    session
        .execute(EditRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            expected_revision: 0,
            action: EditAction::Apply {
                commands: vec![Command::SetVrm {
                    clip_id: id.clone(),
                    standing: pose.clone(),
                }],
            },
        })
        .unwrap();
    let saved = serde_json::to_string(session.history().project()).unwrap();
    let loaded: Project = serde_json::from_str(&saved).unwrap();
    validate(&loaded).unwrap();
    assert_eq!(
        loaded.clips[0].content,
        ClipContent::Vrm {
            standing: pose.clone()
        }
    );
    pose.pose
        .as_mut()
        .unwrap()
        .insert("unknownJoint".into(), [0., 0., 0.]);
    assert!(
        session
            .execute(EditRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: 1,
                action: EditAction::Apply {
                    commands: vec![Command::SetVrm {
                        clip_id: id,
                        standing: pose
                    }]
                }
            })
            .is_err()
    );
    assert_eq!(session.history().revision(), 1);
    let mut history = sungeki_editor::History::new(p.clone()).unwrap();
    let ClipContent::Vrm { standing } = &loaded.clips[0].content else {
        panic!()
    };
    history
        .apply(
            0,
            &[Command::SetVrm {
                clip_id: p.clips[0].id.clone(),
                standing: standing.clone(),
            }],
        )
        .unwrap();
    history.undo(1).unwrap();
    assert_eq!(history.project(), &p);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &loaded);
}
#[test]
fn invalid_joint_angles_are_rejected() {
    for angles in [[181., 0., 0.], [0., f64::NAN, 0.], [0., 0., f64::INFINITY]] {
        assert!(validate_pose(&[("head".into(), angles)].into()).is_err());
    }
    assert!(validate_pose(&[("leftUpperArm".into(), [-180., 0., 180.])].into()).is_ok());
}
