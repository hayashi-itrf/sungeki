//! Resolve relocatable app resources without depending on cwd or user PATH.
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};
use sungeki_protocol::ApiError;

pub(crate) fn file(
    env: Option<OsString>,
    saved: Option<&Path>,
    relative: &str,
) -> Result<PathBuf, ApiError> {
    let executable = std::env::current_exe().map_err(ApiError::io)?;
    resolve(&executable, env, saved, relative)
}
fn resolve(
    executable: &Path,
    env: Option<OsString>,
    saved: Option<&Path>,
    relative: &str,
) -> Result<PathBuf, ApiError> {
    let bundled = executable
        .parent()
        .filter(|p| p.file_name().is_some_and(|n| n == "MacOS"))
        .and_then(Path::parent)
        .map(|p| p.join("Resources/vrm-runtime"));
    // A packaged app always uses its coherent bundled runtime. Explicit developer
    // environment overrides remain available; saved development paths are ignored.
    let selected = env
        .map(PathBuf::from)
        .or_else(|| bundled.filter(|p| p.is_dir()).map(|p| p.join(relative)));
    crate::runtime_settings::file(selected, saved, relative)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relocated_app_prefers_bundle_and_rejects_missing_bundled_files() {
        let fixture = crate::speech_journal_tests::Fixture::new();
        let contents = fixture.0.join("移動した App.app/Contents");
        let root = contents.join("Resources/vrm-runtime");
        std::fs::create_dir_all(root.join("bin")).unwrap();
        let node = root.join("bin/node");
        std::fs::write(&node, b"fixture").unwrap();
        let exe = contents.join("MacOS/sungeki-host");
        assert_eq!(
            resolve(&exe, None, Some(Path::new("/stale/node")), "bin/node").unwrap(),
            node
        );
        assert!(resolve(&exe, None, Some(&node), "missing.ts").is_err());
        assert!(
            resolve(
                &exe,
                Some("/missing/override".into()),
                Some(&node),
                "bin/node"
            )
            .is_err()
        );
        assert_eq!(
            resolve(Path::new("/dev/host"), None, Some(&node), "bin/node").unwrap(),
            node
        );
    }
}
