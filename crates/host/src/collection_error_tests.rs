use super::*;
use std::io::{Error as IoError, ErrorKind};

#[test]
fn capacity_failure_is_structured_for_preflight_assets_and_document() {
    for error in [
        Error::Io(IoError::new(ErrorKind::StorageFull, "required/available")),
        Error::Publish(PublishError::Io(IoError::from_raw_os_error(libc::ENOSPC))),
        Error::Document(sungeki_storage::DocumentError::Io(
            IoError::from_raw_os_error(libc::ENOSPC),
        )),
    ] {
        let result = api(error);
        assert_eq!(result.code, ErrorCode::IoError);
        assert_eq!(result.details["reason"], "insufficient_space");
        assert!(result.message.contains("別の保存先"));
        assert!(result.message.contains("事前確認"));
        assert!(!result.retryable);
    }
    let result = api(Error::Io(IoError::from_raw_os_error(libc::ENOSPC)));
    assert_eq!(result.details["os_error"], libc::ENOSPC.to_string());
}

#[test]
fn permission_and_post_publication_failures_are_not_mislabelled() {
    let result = api(Error::Io(IoError::new(
        ErrorKind::PermissionDenied,
        "denied",
    )));
    assert_eq!(result.message, "denied");
    assert!(!result.details.contains_key("reason"));
    let result = api(Error::Publish(PublishError::DurabilityUncertain {
        path: "/completed".into(),
        source: IoError::from_raw_os_error(libc::ENOSPC),
    }));
    assert_eq!(result.details["publication"], "durability_uncertain");
    assert!(!result.details.contains_key("reason"));
    assert!(result.message.contains("存在します"));
}
