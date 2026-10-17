//! Relative paths belong to persisted documents. Host/editor snapshots remain
//! absolute so Save As, undo and checkpoint state keep their existing meaning.
use crate::DocumentError;
use std::{
    fs,
    path::{Path, PathBuf},
};
use sungeki_project::{AssetLocator, Project};
fn relative(value: &str) -> Result<&Path, DocumentError> {
    if value.is_empty()
        || value.contains(['\0', '\\', ':'])
        || value
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
        || Path::new(value).is_absolute()
    {
        return Err(DocumentError::Invalid(
            "文書内の相対素材パスが不正です".into(),
        ));
    }
    Ok(Path::new(value))
}
fn inside(root: &Path, value: &str) -> Result<PathBuf, DocumentError> {
    let path = relative(value)?;
    let mut current = root.to_path_buf();
    for component in path.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(DocumentError::Invalid(
                    "相対素材パスにsymlinkは使用できません".into(),
                ));
            }
            Ok(_) => {}
            // Missing originals remain repairable asset diagnostics.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(current)
}
pub(crate) fn resolve(project: &mut Project, parent: &Path) -> Result<(), DocumentError> {
    for asset in &mut project.assets {
        if let AssetLocator::ProjectRelative { path } = &asset.locator {
            let target = inside(parent, path)?;
            asset.locator = AssetLocator::Absolute {
                path: target
                    .to_str()
                    .ok_or_else(|| {
                        DocumentError::Invalid("素材パスをUTF-8に変換できません".into())
                    })?
                    .into(),
            };
        }
    }
    Ok(())
}
pub(crate) fn for_save(project: &Project, parent: &Path) -> Project {
    let mut copy = project.clone();
    for asset in &mut copy.assets {
        if let AssetLocator::Absolute { path } = &asset.locator {
            if let Ok(suffix) = Path::new(path).strip_prefix(parent) {
                if let Some(value) = suffix.to_str() {
                    if inside(parent, value).is_ok_and(|target| {
                        target.as_os_str() == std::ffi::OsStr::new(path.as_str())
                    }) {
                        asset.locator = AssetLocator::ProjectRelative { path: value.into() };
                    }
                }
            }
        }
    }
    copy
}
