use sungeki_editor::EditSession;
use sungeki_storage::StoredSession;

// One recently edited document only. Large receipt journals remain fully durable,
// but bypass this optional cache rather than growing its retained input without bound.
// Allow the 64 MiB Undo budget plus current Project and receipt overhead.
const MAX_INPUT_BYTES: usize = 128 * 1024 * 1024;
#[derive(Default)]
pub(crate) struct ParsedCache(Option<(StoredSession, EditSession)>);
impl ParsedCache {
    pub(crate) fn get(&self, stored: &StoredSession) -> Option<EditSession> {
        let (key, session) = self.0.as_ref()?;
        // Includes all persisted bytes, revision and generation, not just Project equality.
        // Storage has already decompressed and verified this read before we reuse parsing.
        (key == stored).then(|| session.clone())
    }
    pub(crate) fn remember(&mut self, stored: StoredSession, session: EditSession) {
        self.0 = (stored.snapshot.len() <= MAX_INPUT_BYTES).then_some((stored, session));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sungeki_editor::{Command, EditAction, EditRequest};
    fn pair() -> (StoredSession, EditSession) {
        let session = EditSession::new(sungeki_project::Project::empty(
            uuid::Uuid::new_v4().to_string(),
            "original".into(),
        ))
        .unwrap();
        let stored = StoredSession {
            id: "a".into(),
            revision: 0,
            version: 0,
            snapshot: session.snapshot().unwrap().into(),
        };
        (stored, session)
    }
    #[test]
    fn cache_requires_exact_persisted_state_and_isolates_candidates() {
        let (stored, session) = pair();
        let mut cache = ParsedCache::default();
        let bytes = stored.snapshot.clone();
        cache.remember(stored, session);
        let mut key = StoredSession {
            id: "a".into(),
            revision: 0,
            version: 0,
            snapshot: bytes.into(),
        };
        let mut candidate = cache.get(&key).unwrap();
        candidate
            .execute(EditRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: 0,
                action: EditAction::Apply {
                    commands: vec![Command::RenameProject {
                        title: "candidate".into(),
                    }],
                },
            })
            .unwrap();
        assert_eq!(
            cache.get(&key).unwrap().history().project().title,
            "original"
        );
        key.version = 1;
        assert!(cache.get(&key).is_none());
        key.version = 0;
        key.revision = 1;
        assert!(cache.get(&key).is_none());
        key.revision = 0;
        key.id = "b".into();
        assert!(cache.get(&key).is_none());
        key.id = "a".into();
        std::sync::Arc::make_mut(&mut key.snapshot).push(0);
        assert!(cache.get(&key).is_none());
    }
    #[test]
    fn oversized_journal_evicts_previous_entry_without_truncating_it() {
        let (stored, session) = pair();
        let mut cache = ParsedCache::default();
        cache.remember(stored, session);
        let (mut large, session) = pair();
        std::sync::Arc::make_mut(&mut large.snapshot).resize(MAX_INPUT_BYTES + 1, 0);
        cache.remember(large, session);
        assert!(cache.0.is_none());
    }
}
