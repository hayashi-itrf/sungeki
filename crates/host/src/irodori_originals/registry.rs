//! Bind every registered reference to the files inspected before dispatch.
use super::invalid;
use crate::original_file;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use sungeki_protocol::ApiError;
use sungeki_speech::irodori::{ReferenceKind, RegisteredVoice};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    name: String,
    reference: ReferenceKind,
    #[serde(deserialize_with = "Option::<String>::deserialize")]
    caption: Option<String>,
    path: Option<PathBuf>,
}
pub(crate) struct Registry {
    pub voices: Vec<RegisteredVoice>,
    pub sha256: String,
}
fn label(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}
pub(crate) fn read(
    path: &Path,
    files: &BTreeMap<String, PathBuf>,
    cancel: &AtomicBool,
) -> Result<Registry, ApiError> {
    let original = original_file::inspect(path, 1024 * 1024, true, cancel)?;
    let entries: Vec<Entry> = serde_json::from_slice(&original.bytes).map_err(|_| invalid())?;
    if entries.is_empty() || entries.len() > 4096 {
        return Err(invalid());
    }
    if files.get("registry").map(PathBuf::as_path) != Some(path) {
        return Err(invalid());
    }
    let mut names = BTreeSet::new();
    let mut voices = Vec::new();
    let mut expected = BTreeMap::from([("registry".to_owned(), path.to_path_buf())]);
    for entry in entries {
        if !label(&entry.id, 128)
            || !label(&entry.name, 1024)
            || !names.insert(entry.id.clone())
            || entry
                .caption
                .as_ref()
                .is_some_and(|v| v.trim().is_empty() || v.len() > 4096 || v.contains('\0'))
        {
            return Err(invalid());
        }
        match (entry.reference, entry.path) {
            (ReferenceKind::None, None) => (),
            (ReferenceKind::Audio | ReferenceKind::Latent, Some(path)) => {
                if !path.is_file()
                    || !path.is_absolute()
                    || path
                        .components()
                        .any(|c| c == std::path::Component::ParentDir)
                    || std::fs::canonicalize(&path).map_err(ApiError::io)? != path
                {
                    return Err(invalid());
                }
                expected.insert(format!("reference:{}", entry.id), path);
            }
            _ => return Err(invalid()),
        }
        voices.push(RegisteredVoice {
            id: entry.id,
            name: entry.name,
            reference: entry.reference,
            caption: entry.caption,
        });
    }
    if &expected != files {
        return Err(invalid());
    }
    Ok(Registry {
        voices,
        sha256: original.sha256,
    })
}
#[cfg(test)]
mod tests;
