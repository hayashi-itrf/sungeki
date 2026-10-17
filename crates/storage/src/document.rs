use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use sungeki_project::{Project, validate};

const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;
#[derive(Debug)]
pub enum DocumentError {
    Io(std::io::Error),
    Invalid(String),
    ExternalModification,
    OutputExists,
}
impl From<std::io::Error> for DocumentError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStamp {
    pub device: u64,
    pub inode: u64,
    pub hash: String,
}
pub struct LoadedDocument {
    pub path: PathBuf,
    pub project: Project,
    pub stamp: FileStamp,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn project_hash(project: &Project) -> Result<String, DocumentError> {
    validate(project).map_err(|e| DocumentError::Invalid(e.to_string()))?;
    Ok(hash(
        &serde_json::to_vec(project).map_err(|e| DocumentError::Invalid(e.to_string()))?,
    ))
}
pub fn document_location(path: &Path) -> Result<PathBuf, DocumentError> {
    if !path.is_absolute()
        || !path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".vvm.json"))
    {
        return Err(DocumentError::Invalid(
            "保存先は絶対パスの.vvm.jsonが必要です".into(),
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| DocumentError::Invalid("親ディレクトリがありません".into()))?
        .canonicalize()?;
    Ok(parent.join(path.file_name().unwrap()))
}
fn read(path: &Path) -> Result<(Vec<u8>, FileStamp), DocumentError> {
    // Reject symlinks without following them, including between metadata and open.
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(DocumentError::Invalid(
            "文書には通常ファイルが必要です（symlink不可）".into(),
        ));
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_DOCUMENT_BYTES {
        return Err(DocumentError::Invalid(
            "文書の種類またはサイズが不正です".into(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_DOCUMENT_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(DocumentError::Invalid("文書サイズの上限です".into()));
    }
    let stamp = FileStamp {
        device: metadata.dev(),
        inode: metadata.ino(),
        hash: hash(&bytes),
    };
    Ok((bytes, stamp))
}
pub fn load_document(path: &Path) -> Result<LoadedDocument, DocumentError> {
    let path = document_location(path)?;
    let (bytes, stamp) = read(&path)?;
    let mut project: Project =
        serde_json::from_slice(&bytes).map_err(|e| DocumentError::Invalid(e.to_string()))?;
    crate::document_paths::resolve(&mut project, path.parent().unwrap())?;
    validate(&project).map_err(|e| DocumentError::Invalid(e.to_string()))?;
    Ok(LoadedDocument {
        path,
        project,
        stamp,
    })
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn temporary(parent: &Path, bytes: &[u8]) -> Result<Temporary, DocumentError> {
    let path = parent.join(format!(".vvm-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    let guard = Temporary(path);
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(guard)
}
fn check(path: &Path, expected: Option<&FileStamp>) -> Result<Option<Vec<u8>>, DocumentError> {
    match read(path) {
        Ok((bytes, stamp)) => match expected {
            Some(expected) if &stamp == expected => Ok(Some(bytes)),
            Some(_) => Err(DocumentError::ExternalModification),
            None => Err(DocumentError::OutputExists),
        },
        Err(DocumentError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            if expected.is_some() {
                Err(DocumentError::ExternalModification)
            } else {
                Ok(None)
            }
        }
        Err(e) => Err(e),
    }
}
pub fn save_document(
    path: &Path,
    project: &Project,
    expected: Option<&FileStamp>,
) -> Result<FileStamp, DocumentError> {
    validate(project).map_err(|e| DocumentError::Invalid(e.to_string()))?;
    let path = document_location(path)?;
    let parent = path.parent().unwrap();
    let persisted = crate::document_paths::for_save(project, parent);
    let bytes =
        serde_json::to_vec_pretty(&persisted).map_err(|e| DocumentError::Invalid(e.to_string()))?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(DocumentError::Invalid("文書サイズの上限です".into()));
    }
    let previous = check(&path, expected)?;
    let temp = temporary(parent, &bytes)?;
    if let Some(previous) = previous {
        let backup = temporary(parent, &previous)?;
        check(&path, expected)?;
        let backup_path = path.with_file_name(format!(
            "{}.bak",
            path.file_name().unwrap().to_string_lossy()
        ));
        fs::rename(&backup.0, backup_path)?;
        check(&path, expected)?;
        fs::rename(&temp.0, &path)?;
    } else {
        // link is an atomic create-if-absent; rename would overwrite a racing creator.
        fs::hard_link(&temp.0, &path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                DocumentError::OutputExists
            } else {
                DocumentError::Io(e)
            }
        })?;
    }
    File::open(parent)?.sync_all()?;
    // Keep the committed inode and known bytes; a later external change is caught on the next save.
    let metadata = fs::metadata(&path)?;
    Ok(FileStamp {
        device: metadata.dev(),
        inode: metadata.ino(),
        hash: hash(&bytes),
    })
}
