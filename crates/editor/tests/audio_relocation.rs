use sungeki_editor::*;
use sungeki_project::*;
fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn locator(path: &str) -> AssetLocator {
    AssetLocator::Absolute { path: path.into() }
}
fn probe() -> AudioProbe {
    AudioProbe {
        encoding: None,
        sha256: "a".repeat(64),
        duration_us: TimeUs(1000),
        sample_rate: 24000,
        channels: 1,
        frames: 24,
        byte_length: 92,
    }
}
fn asset() -> Asset {
    Asset {
        id: id(2),
        name: "音声".into(),
        source: None,
        credit: "作者".into(),
        locator: locator("/original.wav"),
        content_hash: Some("a".repeat(64)),
        probe: AssetProbe::Audio(probe()),
    }
}
fn request(n: u32, revision: u32, action: EditAction) -> EditRequest {
    EditRequest {
        operation_id: id(n),
        expected_revision: revision,
        action,
    }
}
fn apply(n: u32, revision: u32, commands: Vec<Command>) -> EditRequest {
    request(n, revision, EditAction::Apply { commands })
}
fn relocation(source: &str, destination: &str) -> AudioRelocation {
    AudioRelocation {
        asset_id: id(2),
        source: locator(source),
        destination: locator(destination),
        probe: probe(),
    }
}
fn mappings() -> AudioRelocations {
    AudioRelocations::new(vec![relocation(
        "/original.wav",
        "/saved.assets/audio/a.wav",
    )])
    .unwrap()
}
fn fixture() -> (EditSession, EditRequest, EditReceipt) {
    let mut session = EditSession::new(Project::empty(id(1), "作品".into())).unwrap();
    let original_request = apply(10, 0, vec![Command::RegisterAsset { asset: asset() }]);
    let receipt = session.execute(original_request.clone()).unwrap();
    for n in 1..=2 {
        session
            .execute(apply(
                10 + n,
                n,
                vec![Command::RenameProject {
                    title: format!("変更{n}"),
                }],
            ))
            .unwrap();
    }
    session
        .execute(request(
            20,
            3,
            EditAction::Undo {
                expected_operation_id: id(12),
            },
        ))
        .unwrap();
    (session, original_request, receipt)
}
#[test]
fn relocates_current_undo_redo_and_keeps_original_replay_journal() {
    let (mut session, original_request, receipt) = fixture();
    let counts = (session.history().undo_len(), session.history().redo_len());
    assert!(session.relocate_audio(4, &mappings()).unwrap());
    assert_eq!(session.history().revision(), 5);
    assert_eq!(
        (session.history().undo_len(), session.history().redo_len()),
        counts
    );
    for project in session.history().retained_projects() {
        for asset in &project.assets {
            assert_eq!(asset.locator, locator("/saved.assets/audio/a.wav"));
        }
    }
    assert_eq!(
        session.recorded_request(&original_request.operation_id),
        Some(&original_request)
    );
    assert_eq!(session.execute(original_request).unwrap(), receipt);
    assert_eq!(session.history().revision(), 5);
    let before_repeat = session.snapshot().unwrap();
    assert!(!session.relocate_audio(5, &mappings()).unwrap());
    assert_eq!(session.snapshot().unwrap(), before_repeat);
    session
        .execute(request(
            21,
            5,
            EditAction::Undo {
                expected_operation_id: id(11),
            },
        ))
        .unwrap();
    assert_eq!(session.history().project().title, "作品");
    assert_eq!(
        session.history().project().assets[0].locator,
        locator("/saved.assets/audio/a.wav")
    );
    session
        .execute(request(
            22,
            6,
            EditAction::Redo {
                expected_operation_id: id(11),
            },
        ))
        .unwrap();
    assert_eq!(session.history().project().title, "変更1");
    let restored = EditSession::restore(&session.snapshot().unwrap()).unwrap();
    assert_eq!(restored.snapshot().unwrap(), session.snapshot().unwrap());
}
#[test]
fn concurrent_relink_or_changed_original_is_not_overwritten() {
    for changed_hash in [false, true] {
        let (mut session, _, _) = fixture();
        let mut changed = session.history().project().clone();
        if changed_hash {
            let AssetProbe::Audio(p) = &mut changed.assets[0].probe else {
                panic!()
            };
            p.sha256 = "b".repeat(64);
            changed.assets[0].content_hash = Some("b".repeat(64));
        } else {
            changed.assets[0].locator = locator("/user-relinked.wav");
        }
        session
            .execute(apply(
                30,
                4,
                vec![Command::ReplaceProject {
                    project: changed.clone(),
                    checkpoint_id: None,
                }],
            ))
            .unwrap();
        assert!(session.relocate_audio(5, &mappings()).unwrap()); // Retained historical originals still move.
        assert_eq!(session.history().project(), &changed);
        session
            .execute(request(
                31,
                6,
                EditAction::Undo {
                    expected_operation_id: id(30),
                },
            ))
            .unwrap();
        assert_eq!(
            session.history().project().assets[0].locator,
            locator("/saved.assets/audio/a.wav")
        );
    }
}
#[test]
fn stale_revision_and_revision_overflow_leave_every_byte_unchanged() {
    let (mut session, _, _) = fixture();
    let before = session.snapshot().unwrap();
    assert!(session.relocate_audio(3, &mappings()).is_err());
    assert_eq!(session.snapshot().unwrap(), before);
    let mut encoded: serde_json::Value = serde_json::from_slice(&before).unwrap();
    encoded["history"]["revision"] = serde_json::json!(u32::MAX);
    let mut session = EditSession::restore(&serde_json::to_vec(&encoded).unwrap()).unwrap();
    let before = session.snapshot().unwrap();
    assert!(session.relocate_audio(u32::MAX, &mappings()).is_err());
    assert_eq!(session.snapshot().unwrap(), before);
}
#[test]
fn ambiguous_non_idempotent_and_invalid_maps_are_rejected() {
    for map in [
        vec![relocation("/a", "/b"), relocation("/a", "/c")],
        vec![relocation("/a", "/b"), relocation("/b", "/c")],
        vec![relocation("/a", "/b"), relocation("/b", "/a")],
        vec![relocation("/a", "relative")],
    ] {
        assert!(AudioRelocations::new(map).is_err());
    }
    assert!(AudioRelocations::new(vec![relocation("/a", "/b"), relocation("/b", "/b")]).is_ok());
    let duplicate = relocation("/a", "/b");
    assert!(AudioRelocations::new(vec![duplicate.clone(), duplicate]).is_ok());
}
