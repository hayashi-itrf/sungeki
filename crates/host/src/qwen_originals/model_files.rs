//! Inventory every local model file, including HF snapshot links to cached blobs.
//! Re-enumerate at launch and after synthesis so added/retargeted shards invalidate
//! the captured manifest. Directory links are rejected to avoid recursive aliases.
use super::invalid;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use sungeki_protocol::{ApiError, ErrorCode};

pub(crate) fn discover(
    root: &Path,
    cancel: &AtomicBool,
) -> Result<BTreeMap<String, PathBuf>, ApiError> {
    if !root.is_absolute()
        || root
            .components()
            .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(invalid());
    }
    let root = std::fs::canonicalize(root).map_err(ApiError::io)?;
    if !root.is_dir() {
        return Err(invalid());
    }
    let mut files = BTreeMap::new();
    let mut pending = vec![(root.clone(), 0)];
    let mut entries = 0usize;
    while let Some((directory, depth)) = pending.pop() {
        if depth > 32 {
            return Err(invalid());
        }
        for entry in std::fs::read_dir(directory).map_err(ApiError::io)? {
            if cancel.load(Ordering::Acquire) {
                return Err(ApiError::new(
                    ErrorCode::Cancelled,
                    "Qwenモデル一覧の取得を取り消しました",
                ));
            }
            entries += 1;
            if entries > 65536 {
                return Err(invalid());
            }
            let entry = entry.map_err(ApiError::io)?;
            let path = entry.path();
            let kind = entry.file_type().map_err(ApiError::io)?;
            if kind.is_dir() {
                pending.push((path, depth + 1));
                continue;
            }
            if !kind.is_file() && !kind.is_symlink() {
                return Err(invalid());
            }
            let target = std::fs::canonicalize(&path).map_err(ApiError::io)?;
            if !std::fs::symlink_metadata(&target)
                .map_err(ApiError::io)?
                .is_file()
            {
                return Err(invalid());
            }
            let name = path
                .strip_prefix(&root)
                .map_err(|_| invalid())?
                .to_str()
                .ok_or_else(invalid)?
                .to_owned();
            if name.len() > 1024 || name.chars().any(char::is_control) {
                return Err(invalid());
            }
            files.insert(name, target);
        }
    }
    if !files.contains_key("config.json") {
        return Err(invalid());
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::symlink};
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> Fixture {
        let root = std::env::temp_dir().join(format!("qwen-model-files-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("snapshot/tokenizer")).unwrap();
        fs::write(root.join("snapshot/config.json"), "{}").unwrap();
        fs::write(root.join("snapshot/tokenizer/config.json"), "{}").unwrap();
        Fixture(fs::canonicalize(root).unwrap())
    }
    #[test]
    fn inventories_nested_files_and_snapshot_blob_links_and_detects_retargeting() {
        let f = fixture();
        let root = f.0.join("snapshot");
        fs::write(f.0.join("blob-a"), "weights-a").unwrap();
        fs::write(f.0.join("blob-b"), "weights-b").unwrap();
        symlink("../blob-a", root.join("model.safetensors")).unwrap();
        let first = discover(&root, &AtomicBool::new(false)).unwrap();
        assert_eq!(first.len(), 3);
        assert_eq!(first["model.safetensors"], f.0.join("blob-a"));
        fs::remove_file(root.join("model.safetensors")).unwrap();
        symlink("../blob-b", root.join("model.safetensors")).unwrap();
        assert_ne!(first, discover(&root, &AtomicBool::new(false)).unwrap());
        fs::write(root.join("additional.safetensors"), "shard").unwrap();
        assert_eq!(discover(&root, &AtomicBool::new(false)).unwrap().len(), 4);
    }
    #[test]
    fn rejects_directory_links_missing_config_and_cancellation() {
        let f = fixture();
        let root = f.0.join("snapshot");
        assert_eq!(
            discover(&root, &AtomicBool::new(true)).unwrap_err().code,
            ErrorCode::Cancelled
        );
        symlink(&root, root.join("loop")).unwrap();
        assert!(discover(&root, &AtomicBool::new(false)).is_err());
        fs::remove_file(root.join("loop")).unwrap();
        fs::remove_file(root.join("config.json")).unwrap();
        assert!(discover(&root, &AtomicBool::new(false)).is_err());
    }
}
