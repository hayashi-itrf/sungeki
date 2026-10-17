use crate::Command;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use sungeki_project::{Project, ValidationError};

pub fn has_batch_references(commands: &[Command]) -> bool {
    commands
        .iter()
        .filter_map(creation_id)
        .any(|id| id.starts_with('$'))
}
pub(crate) fn creation_id(command: &Command) -> Option<&str> {
    match command {
        Command::AddTrack { track } => Some(&track.id),
        Command::SetCharacter { character } => Some(&character.id),
        Command::RegisterAsset { asset } => Some(&asset.id),
        Command::AddScene { scene } => Some(&scene.id),
        Command::AddGroup { group } => Some(&group.id),
        Command::AddClip { clip } => Some(&clip.id),
        _ => None,
    }
}
/// Stable across preview pagination and actual execution at the same revision.
/// Original input is journalled; operation retries return its persisted receipt.
pub(crate) fn allocate(
    project: &Project,
    revision: u32,
    commands: &[Command],
) -> Result<BTreeMap<String, String>, ValidationError> {
    let input = serde_json::to_vec(&(project.id.as_str(), revision, commands))
        .map_err(|e| ValidationError::new(e.to_string()))?;
    let input_digest = Sha256::digest(&input);
    let mut ids = BTreeMap::new();
    for name in commands
        .iter()
        .filter_map(creation_id)
        .filter_map(|id| id.strip_prefix('$'))
    {
        let mut hash = Sha256::new();
        // Persistent hash domain: keep stable across product renames.
        hash.update(b"vvm-next-batch-reference-v1\0");
        hash.update(input_digest);
        hash.update(name.as_bytes());
        let digest = hash.finalize();
        let mut bytes: [u8; 16] = digest[..16].try_into().unwrap();
        bytes[6] = (bytes[6] & 0x0f) | 0x80;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        ids.insert(name.to_owned(), uuid::Uuid::from_bytes(bytes).to_string());
    }
    Ok(ids)
}
