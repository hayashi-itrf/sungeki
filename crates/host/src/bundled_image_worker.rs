//! Locate the small application worker relative to the bundled host, never CWD.
use std::path::{Path, PathBuf};
use sungeki_protocol::ApiError;

pub(crate) fn resolve(
    override_path: Option<PathBuf>,
    saved: Option<&Path>,
    executable: Option<&Path>,
) -> Result<PathBuf, ApiError> {
    let bundled = executable.and_then(|exe| {
        let macos = exe.parent()?;
        let contents = macos.parent()?;
        if macos.file_name()? != "MacOS" || contents.file_name()? != "Contents" {
            return None;
        }
        Some(contents.join("Resources/image-worker/render_frame.py"))
    });
    // Explicit invalid settings must still fail rather than silently use another worker.
    super::file(
        override_path,
        saved.or(bundled.as_deref()),
        "SUNGEKI_RENDER_WORKER",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundle_fallback_is_relative_to_host_and_explicit_paths_take_precedence() {
        let root = std::env::temp_dir().join(format!("worker-location-{}", uuid::Uuid::new_v4()));
        let contents = root.join("Example.app/Contents");
        let worker = contents.join("Resources/image-worker/render_frame.py");
        std::fs::create_dir_all(worker.parent().unwrap()).unwrap();
        std::fs::write(&worker, b"fixture").unwrap();
        let exe = contents.join("MacOS/sungeki-host");
        assert_eq!(resolve(None, None, Some(&exe)).unwrap(), worker);
        let explicit = root.join("custom.py");
        std::fs::write(&explicit, b"custom").unwrap();
        assert_eq!(
            resolve(None, Some(&explicit), Some(&exe)).unwrap(),
            explicit
        );
        assert_eq!(
            resolve(Some(explicit.clone()), Some(&worker), Some(&exe)).unwrap(),
            explicit
        );
        let missing = root.join("missing.py");
        assert!(resolve(None, Some(&missing), Some(&exe)).is_err());
        assert!(resolve(Some(missing), Some(&explicit), Some(&exe)).is_err());
        assert!(resolve(None, None, Some(&root.join("target/release/sungeki-host"))).is_err());
        assert!(resolve(None, None, None).is_err());
        std::fs::remove_file(&worker).unwrap();
        assert!(resolve(None, None, Some(&exe)).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
