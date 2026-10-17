use super::*;
use std::sync::atomic::Ordering;

#[test]
fn profile_shutdown_rejects_all_discovery_without_touching_configuration() {
    let root = std::env::temp_dir().join(format!("profile-shutdown-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let shared = Arc::new(Mutex::new(Sessions::open(&root.join("db")).unwrap()));
    // A request that already captured the flag observes the same shutdown signal.
    let (_, active_flag) = profile_inspection(&shared).unwrap();
    shared
        .lock()
        .unwrap()
        .profile_inspection_cancel
        .store(true, Ordering::Release);
    let missing = root.join("must-not-be-read");
    assert_eq!(
        crate::original_file::inspect(&missing, 1024, true, &active_flag)
            .err()
            .unwrap()
            .code,
        ErrorCode::Cancelled
    );
    for request in [
        SpeechJobRequest::ListQwenProfiles,
        SpeechJobRequest::ListIrodoriProfiles,
        SpeechJobRequest::ListSbv2Profiles,
        SpeechJobRequest::GetQwenProfile {
            args: GetQwenProfileArgs {
                profile_id: "local".into(),
            },
        },
        SpeechJobRequest::GetIrodoriProfile {
            args: GetIrodoriProfileArgs {
                profile_id: "local".into(),
            },
        },
        SpeechJobRequest::GetSbv2Profile {
            args: GetSbv2ProfileArgs {
                profile_id: "local".into(),
            },
        },
    ] {
        assert_eq!(
            execute(&shared, request).unwrap_err().code,
            ErrorCode::Cancelled
        );
    }
    assert!(
        shared
            .lock()
            .unwrap()
            .store
            .list_jobs(None, None, None, 100)
            .unwrap()
            .is_empty()
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
