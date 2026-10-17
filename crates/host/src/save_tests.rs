use crate::sessions::Sessions;
use sungeki_protocol::*;
use sungeki_storage::{DocumentBinding, save_document};
#[test]
fn edit_during_save_stays_dirty_and_undo_reaches_saved_content() {
    let dir = std::env::temp_dir().join(format!("vvm-save-edit-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    {
        let mut sessions = Sessions::open(&dir.join("db")).unwrap();
        let SessionResponse::Snapshot { snapshot } = sessions
            .request(SessionRequest::Create {
                title: "保存する版".into(),
            })
            .unwrap()
        else {
            panic!()
        };
        let id = snapshot.session_id;
        let plan = sessions
            .prepare_save(
                id.clone(),
                Some(dir.join("test.vvm.json").to_string_lossy().into()),
                0,
            )
            .unwrap();
        assert!(
            sessions
                .prepare_save(
                    id.clone(),
                    Some(dir.join("other.vvm.json").to_string_lossy().into()),
                    0
                )
                .is_err()
        );
        let op = uuid::Uuid::new_v4().to_string();
        sessions
            .request(SessionRequest::Edit {
                session_id: id.clone(),
                edit: EditRequest {
                    operation_id: op.clone(),
                    expected_revision: 0,
                    action: EditAction::Apply {
                        commands: vec![sungeki_editor::Command::RenameProject {
                            title: "保存中の変更".into(),
                        }],
                    },
                },
            })
            .unwrap();
        let stamp = save_document(&plan.path, &plan.project, plan.expected.as_ref()).unwrap();
        sessions.release_save(&plan);
        let SessionResponse::Snapshot { snapshot } = sessions
            .finish_save(DocumentBinding {
                session_id: id.clone(),
                path: plan.path.to_string_lossy().into(),
                stamp,
                saved_content_hash: plan.content_hash,
            })
            .unwrap()
        else {
            panic!()
        };
        assert!(snapshot.dirty);
        assert_eq!(snapshot.project.title, "保存中の変更");
        assert_eq!(snapshot.revision, 1);
        let document = sungeki_storage::load_document(&plan.path).unwrap();
        assert_eq!(document.project.title, "保存する版");
        let SessionResponse::Edited { snapshot, .. } = sessions
            .request(SessionRequest::Edit {
                session_id: id,
                edit: EditRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    expected_revision: 1,
                    action: EditAction::Undo {
                        expected_operation_id: op,
                    },
                },
            })
            .unwrap()
        else {
            panic!()
        };
        assert!(!snapshot.dirty);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
