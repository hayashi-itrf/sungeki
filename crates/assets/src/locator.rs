use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};
use sungeki_project::AssetLocator;

/// Explicit roots only: no current-directory or process-environment fallback.
#[derive(Debug, Default)]
pub struct AssetRoots {
    pub document_path: Option<PathBuf>,
    pub libraries: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetLocationError {
    InvalidPath,
    UnsavedProject,
    MissingLibrary(String),
    InvalidRoot,
    NonUnicodePath,
}
impl std::fmt::Display for AssetLocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath => write!(f, "素材の参照先が不正です"),
            Self::UnsavedProject => write!(f, "作品が未保存のため相対素材の基準がありません"),
            Self::MissingLibrary(id) => write!(f, "素材ライブラリが見つかりません: {id}"),
            Self::InvalidRoot => write!(f, "素材の基準には絶対パスが必要です"),
            Self::NonUnicodePath => write!(f, "素材パスをUTF-8で保存できません"),
        }
    }
}
impl std::error::Error for AssetLocationError {}

fn document_directory(path: Option<&Path>) -> Result<&Path, AssetLocationError> {
    let path = path.ok_or(AssetLocationError::UnsavedProject)?;
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(AssetLocationError::InvalidRoot);
    }
    path.parent().ok_or(AssetLocationError::InvalidRoot)
}
fn relative(path: &str) -> Result<&Path, AssetLocationError> {
    let value = Path::new(path);
    if path.trim().is_empty()
        || path.contains('\0')
        || value.is_absolute()
        || value
            .components()
            .any(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
    {
        return Err(AssetLocationError::InvalidPath);
    }
    Ok(value)
}

/// Resolve even absent files; missing media is a repairable diagnostic, not invalid Project data.
/// Preserve `..` rather than normalizing across symlinks and silently changing the target.
pub fn resolve(locator: &AssetLocator, roots: &AssetRoots) -> Result<PathBuf, AssetLocationError> {
    match locator {
        AssetLocator::Absolute { path } => {
            let value = Path::new(path);
            if path.trim().is_empty() || path.contains('\0') || !value.is_absolute() {
                return Err(AssetLocationError::InvalidPath);
            }
            Ok(value.to_owned())
        }
        AssetLocator::ProjectRelative { path } => {
            Ok(document_directory(roots.document_path.as_deref())?.join(relative(path)?))
        }
        AssetLocator::Library { library_id, path } => {
            let root = roots
                .libraries
                .get(library_id)
                .ok_or_else(|| AssetLocationError::MissingLibrary(library_id.clone()))?;
            if !root.is_absolute() {
                return Err(AssetLocationError::InvalidRoot);
            }
            Ok(root.join(relative(path)?))
        }
    }
}

/// Preserve the exact old target on Save As, including missing files and symlink/.. paths.
/// Existing directory roots are canonicalized before deriving a relative reference.
/// Unavailable roots fall back to explicit absolute references; no source media is copied.
/// Generated project-owned media relocation is a separate host/storage transaction.
pub fn rebase(
    locator: &AssetLocator,
    old: &AssetRoots,
    new_document: &Path,
) -> Result<AssetLocator, AssetLocationError> {
    let new_directory = document_directory(Some(new_document))?;
    match locator {
        AssetLocator::ProjectRelative { path } => {
            let target = resolve(locator, old)?;
            if document_directory(old.document_path.as_deref())? == new_directory {
                return Ok(locator.clone());
            }
            let old_directory = document_directory(old.document_path.as_deref())?;
            if let (Ok(from), Ok(to)) = (new_directory.canonicalize(), old_directory.canonicalize())
            {
                if let Some(prefix) = relative_directory(&from, &to) {
                    let rebased = prefix.join(relative(path)?);
                    return Ok(AssetLocator::ProjectRelative {
                        path: rebased
                            .to_str()
                            .ok_or(AssetLocationError::NonUnicodePath)?
                            .to_owned(),
                    });
                }
            }
            Ok(AssetLocator::Absolute {
                path: target
                    .to_str()
                    .ok_or(AssetLocationError::NonUnicodePath)?
                    .to_owned(),
            })
        }
        // Library identifiers retain their explicit, portable root rather than becoming CWD-relative.
        AssetLocator::Library { library_id, path } => {
            if library_id.trim().is_empty() {
                return Err(AssetLocationError::InvalidPath);
            }
            relative(path)?;
            Ok(locator.clone())
        }
        AssetLocator::Absolute { .. } => {
            resolve(locator, old)?;
            Ok(locator.clone())
        }
    }
}

// Inputs are canonical directory roots, so parent traversal cannot cross a hidden symlink.
fn relative_directory(from: &Path, to: &Path) -> Option<PathBuf> {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    if common == 0 {
        return None;
    }
    let mut result = PathBuf::new();
    for _ in common..from.len() {
        result.push("..");
    }
    for part in &to[common..] {
        result.push(part.as_os_str());
    }
    Some(result)
}
