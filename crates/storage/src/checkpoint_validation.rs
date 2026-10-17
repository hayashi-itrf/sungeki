use crate::{DocumentError, project_hash};
use sungeki_project::{Checkpoint, CheckpointAsset, Project};

pub(crate) fn manifest(project: &Project) -> Vec<CheckpointAsset> {
    project
        .assets
        .iter()
        .map(|asset| CheckpointAsset {
            asset_id: asset.id.clone(),
            locator: asset.locator.clone(),
            content_hash: asset.content_hash.clone(),
        })
        .collect()
}
pub(crate) fn uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.hyphenated().to_string() == value)
}
pub(crate) fn metadata(id: &str, name: &str, created_at: u64) -> bool {
    uuid(id)
        && !name.trim().is_empty()
        && name.chars().count() <= 200
        && created_at <= 9_007_199_254_740_991
}
pub(crate) fn validate(checkpoint: &Checkpoint) -> Result<(), DocumentError> {
    let s = &checkpoint.summary;
    if s.project_id != checkpoint.project.id
        || !metadata(&s.id, &s.name, s.created_at)
        || project_hash(&checkpoint.project)? != s.content_hash
        || checkpoint.asset_manifest != manifest(&checkpoint.project)
    {
        return Err(DocumentError::Invalid(
            "保存時点の内容・素材一覧・識別子が一致しません".into(),
        ));
    }
    Ok(())
}
