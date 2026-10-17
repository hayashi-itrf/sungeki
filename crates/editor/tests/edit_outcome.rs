use sungeki_editor::*;
use sungeki_project::*;
fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn request(n: u32, revision: u32, commands: Vec<Command>) -> EditRequest {
    EditRequest {
        operation_id: id(n),
        expected_revision: revision,
        action: EditAction::Apply { commands },
    }
}
fn track(n: u32) -> Track {
    Track {
        id: id(n),
        name: format!("Track {n}"),
        visible: true,
        muted: false,
        kind: TrackKind::Visual,
    }
}
#[test]
fn net_changes_reordering_and_noop_are_reported_without_media_claims() {
    let mut p = Project::empty(id(1), "Original".into());
    p.tracks = vec![track(2), track(3)];
    let mut session = EditSession::new(p).unwrap();
    let receipt = session
        .execute(request(
            10,
            0,
            vec![
                Command::AddTrack { track: track(4) },
                Command::ReorderTracks {
                    track_ids: vec![id(3), id(2), id(4)],
                },
                Command::RenameProject {
                    title: "Changed".into(),
                },
            ],
        ))
        .unwrap();
    let summary = receipt.summary.unwrap();
    assert_eq!(
        (summary.added, summary.removed, summary.modified),
        (1, 0, 0)
    );
    assert_eq!(summary.reordered_collections, ["tracks"]);
    assert_eq!(summary.project_fields, ["title"]);
    let ids = receipt.changed_ids.unwrap();
    assert_eq!(ids["tracks"], [id(2), id(3), id(4)]);
    assert_eq!(ids["project"], [id(1)]);
    let diagnostics = receipt.diagnostics.unwrap();
    assert_eq!(diagnostics.summary.passed, 1);
    assert_eq!(diagnostics.summary.not_run, 5);
    assert!(!diagnostics.summary.complete);
    assert!(diagnostics.issues.is_empty());
    let no_op = session
        .execute(request(
            11,
            1,
            vec![
                Command::RenameProject {
                    title: "Temporary".into(),
                },
                Command::RenameProject {
                    title: "Changed".into(),
                },
            ],
        ))
        .unwrap();
    assert!(no_op.no_op);
    assert_eq!(no_op.revision, 1);
    assert!(no_op.changed_ids.unwrap().is_empty());
    assert_eq!(no_op.summary.unwrap(), EditSummary::default());
}
#[test]
fn cascading_deletion_undo_redo_and_recovery_keep_original_outcomes() {
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
    p.scenes.clear();
    p.groups.clear();
    for clip in &mut p.clips {
        clip.scene_id = None;
    }
    let clip_id = p.clips[0].id.clone();
    p.groups.push(Group {
        id: id(70),
        name: "Single".into(),
        clip_ids: vec![clip_id.clone()],
    });
    let mut session = EditSession::new(p.clone()).unwrap();
    let deletion = request(
        10,
        0,
        vec![Command::DeleteClip {
            clip_id: clip_id.clone(),
        }],
    );
    let receipt = session.execute(deletion.clone()).unwrap();
    assert_eq!(receipt.summary.as_ref().unwrap().removed, 2);
    assert_eq!(receipt.changed_ids.as_ref().unwrap()["clips"], [clip_id]);
    assert_eq!(receipt.changed_ids.as_ref().unwrap()["groups"], [id(70)]);
    assert!(
        receipt
            .summary
            .as_ref()
            .unwrap()
            .reordered_collections
            .is_empty()
    );
    let mut restored = EditSession::restore(&session.snapshot().unwrap()).unwrap();
    let undo = restored
        .execute(EditRequest {
            operation_id: id(11),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: id(10),
            },
        })
        .unwrap();
    assert_eq!(undo.summary.as_ref().unwrap().added, 2);
    assert_eq!(undo.changed_ids, receipt.changed_ids);
    assert_eq!(restored.history().project(), &p);
    assert_eq!(restored.execute(deletion).unwrap(), receipt);
    let redo = restored
        .execute(EditRequest {
            operation_id: id(12),
            expected_revision: 2,
            action: EditAction::Redo {
                expected_operation_id: id(10),
            },
        })
        .unwrap();
    assert_eq!(redo.summary, receipt.summary);
    assert_eq!(redo.changed_ids, receipt.changed_ids);
}
#[test]
fn modifications_and_insertion_are_distinct_from_reordering() {
    let mut p = Project::empty(id(1), "Original".into());
    p.tracks = vec![track(2), track(3)];
    let mut session = EditSession::new(p).unwrap();
    let result = session
        .execute(request(
            10,
            0,
            vec![
                Command::AddTrack { track: track(4) },
                Command::ReorderTracks {
                    track_ids: vec![id(4), id(2), id(3)],
                },
                Command::SetTrackProperties {
                    track_id: id(2),
                    name: "Renamed".into(),
                    visible: true,
                    muted: true,
                },
            ],
        ))
        .unwrap();
    let summary = result.summary.unwrap();
    assert_eq!((summary.added, summary.modified), (1, 1));
    assert!(summary.reordered_collections.is_empty());
    assert_eq!(result.changed_ids.unwrap()["tracks"], [id(2), id(4)]);
}
