use sungeki_storage::*;
#[test]
fn bindings_survive_reopen_and_reject_path_or_inode_ownership_conflicts() {
    let dir = std::env::temp_dir().join(format!("vvm-binding-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let db = dir.join("db");
    let original = DocumentBinding {
        session_id: "one".into(),
        path: "/a.vvm.json".into(),
        stamp: FileStamp {
            device: 1,
            inode: 2,
            hash: "disk".into(),
        },
        saved_content_hash: "project".into(),
    };
    {
        let mut store = RecoveryStore::open(&db).unwrap();
        for id in ["one", "two"] {
            store
                .commit(
                    None,
                    &StoredSession {
                        id: id.into(),
                        revision: 0,
                        version: 0,
                        snapshot: vec![].into(),
                    },
                )
                .unwrap();
        }
        store.bind_document(&original).unwrap();
        let mut other = original.clone();
        other.session_id = "two".into();
        assert!(matches!(
            store.bind_document(&other),
            Err(StorageError::Conflict)
        ));
        other.path = "/hardlink.vvm.json".into();
        assert!(matches!(
            store.bind_document(&other),
            Err(StorageError::Conflict)
        ));
        assert!(store.document_binding("two").unwrap().is_none());
    }
    {
        let store = RecoveryStore::open(&db).unwrap();
        assert_eq!(
            store.document_binding("one").unwrap(),
            Some(original.clone())
        );
        assert_eq!(
            store
                .document_owner("/hardlink.vvm.json", Some(&original.stamp))
                .unwrap(),
            Some("one".into())
        );
        assert_eq!(store.load("one").unwrap().unwrap().revision, 0);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
