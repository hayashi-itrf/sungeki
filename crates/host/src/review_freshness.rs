use crate::review_jobs::Runtime;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, atomic::Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use sungeki_project::Asset;
use sungeki_protocol::{AssetFreshness, AssetFreshnessStatus as Status};

const MAX_ENTRIES: usize = 64;
const REFRESH_AFTER: Duration = Duration::from_secs(5);
struct Entry {
    value: AssetFreshness,
    observed: Instant,
}
pub(crate) struct Cache {
    entries: Mutex<HashMap<String, Entry>>,
    slots: Arc<tokio::sync::Semaphore>,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            entries: Default::default(),
            slots: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}
struct Active(Runtime);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.notify();
    }
}
impl Cache {
    /// No file reads, waits, or decoding occur on the request/editing thread.
    /// A busy scan slot leaves the prior observation visible; no unbounded queue.
    pub fn observe(
        &self,
        runtime: &Runtime,
        id: &str,
        assets: &[Asset],
        expected: Option<&str>,
    ) -> AssetFreshness {
        let Some(expected) = expected else {
            return AssetFreshness::default();
        };
        if assets.is_empty() {
            return AssetFreshness {
                status: Status::Matching,
                message: Some("登録された画像・音声・PSD・動画はありません".into()),
                ..Default::default()
            };
        }
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let prior = entries
            .get(id)
            .map(|entry| entry.value.clone())
            .unwrap_or_default();
        if entries
            .get(id)
            .is_some_and(|entry| entry.value.checking || entry.observed.elapsed() < REFRESH_AFTER)
        {
            return prior;
        }
        if runtime.active.load(Ordering::SeqCst) >= 8 {
            return prior;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return prior;
        };
        let Ok(slot) = self.slots.clone().try_acquire_owned() else {
            return prior;
        };
        if !entries.contains_key(id) && entries.len() >= MAX_ENTRIES {
            let oldest = entries
                .iter()
                .filter(|(_, entry)| !entry.value.checking)
                .min_by_key(|(_, entry)| entry.observed)
                .map(|(id, _)| id.clone());
            if let Some(oldest) = oldest {
                entries.remove(&oldest);
            } else {
                return prior;
            }
        }
        let value = AssetFreshness {
            checking: true,
            ..prior
        };
        entries.insert(
            id.into(),
            Entry {
                value: value.clone(),
                observed: Instant::now(),
            },
        );
        runtime.active.fetch_add(1, Ordering::SeqCst);
        let runtime = runtime.clone();
        let id = id.to_owned();
        let assets = assets.to_vec();
        let expected = expected.to_owned();
        handle.spawn(async move {
            let _active = Active(runtime.clone());
            let _slot = slot;
            let outcome = tokio::task::spawn_blocking(move || {
                sungeki_review::current_source_fingerprint(&assets, 256 * 1024 * 1024)
            })
            .await;
            let (status, message) = match outcome {
                Ok(Ok(actual)) if actual == expected => (Status::Matching, None),
                Ok(Ok(_)) => (
                    Status::Changed,
                    Some("検査後に画像・音声・PSD・動画の内容が変わっています".into()),
                ),
                Ok(Err(sungeki_assets::ProbeError::Io(error)))
                    if error.kind() == std::io::ErrorKind::NotFound =>
                {
                    (
                        Status::Changed,
                        Some("検査時にあった画像・音声・PSD・動画が見つかりません".into()),
                    )
                }
                Ok(Err(error)) => (Status::Unavailable, Some(error.to_string())),
                Err(error) => (
                    Status::Unavailable,
                    Some(format!("素材の再確認を完了できません: {error}")),
                ),
            };
            let checked_at_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|time| u64::try_from(time.as_millis()).ok());
            // Release the cache mutex before notifying waiters (which may hold the signal mutex).
            {
                let mut entries = runtime
                    .freshness
                    .entries
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let last_known_changed = match status {
                    Status::Changed => true,
                    Status::Matching => false,
                    _ => entries
                        .get(&id)
                        .is_some_and(|entry| entry.value.last_known_changed),
                };
                entries.insert(
                    id,
                    Entry {
                        value: AssetFreshness {
                            status,
                            last_known_changed,
                            checking: false,
                            checked_at_ms,
                            message,
                        },
                        observed: Instant::now(),
                    },
                );
            }
        });
        value
    }
}

#[cfg(test)]
#[path = "review_freshness_tests.rs"]
mod tests;
