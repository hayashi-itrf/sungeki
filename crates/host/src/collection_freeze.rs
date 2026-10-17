//! Read history while the common session lock is held; never enumerate in worker.
use super::{Error, history::FrozenCheckpoint};
use sungeki_storage::RecoveryStore;

pub(super) fn capture(
    store: &RecoveryStore,
    project_id: &str,
) -> Result<Vec<FrozenCheckpoint>, Error> {
    let mut result = Vec::new();
    let mut after = None;
    let mut total = 0usize;
    loop {
        let page = store
            .list_checkpoints(project_id, after.as_deref(), 51)
            .map_err(super::worker::invalid)?;
        if page.is_empty() {
            return Ok(result);
        }
        for summary in page {
            if result.len() == 127 {
                return Err(Error::Limit(
                    "履歴付き収集は最大127保存時点です。履歴を省略して続行はしません".into(),
                ));
            }
            let stored = store
                .checkpoint(&summary.id)
                .map_err(super::worker::invalid)?
                .ok_or_else(|| Error::Invalid("収集中の保存時点がありません".into()))?;
            if stored.checkpoint.summary != summary {
                return Err(Error::Invalid("収集中の保存時点が変更されました".into()));
            }
            total = total
                .checked_add(
                    serde_json::to_vec(&stored.checkpoint)
                        .map_err(super::worker::invalid)?
                        .len(),
                )
                .filter(|total| *total <= 64 * 1024 * 1024)
                .ok_or_else(|| Error::Limit("履歴の合計容量上限です".into()))?;
            after = Some(summary.id);
            result.push(FrozenCheckpoint {
                collected_id: uuid::Uuid::new_v4().to_string(),
                source: stored.checkpoint,
            });
        }
    }
}

pub(super) fn unchanged(
    store: &RecoveryStore,
    input: &super::record::Input,
) -> Result<bool, Error> {
    if !input.include_checkpoints {
        return Ok(true);
    }
    let mut after = None;
    let mut index = 0;
    loop {
        let page = store
            .list_checkpoints(&input.project.id, after.as_deref(), 51)
            .map_err(super::worker::invalid)?;
        if page.is_empty() {
            return Ok(index == input.checkpoints.len());
        }
        for summary in page {
            if input
                .checkpoints
                .get(index)
                .is_none_or(|c| c.source.summary != summary)
            {
                return Ok(false);
            }
            index += 1;
            after = Some(summary.id);
        }
    }
}
