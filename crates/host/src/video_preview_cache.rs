//! Host-local, bounded prepared snapshots. The media admission serializes use.
use super::{Work, guard::OriginalStamp};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use sungeki_media::video_probe::frames::PreparedVideo;
use sungeki_protocol::ApiError;
pub(super) struct Prepared {
    pub video: PreparedVideo,
    pub guards: Vec<(PathBuf, OriginalStamp)>,
    pub _work: Work,
}
impl Prepared {
    pub fn verify(&self) -> Result<(), ApiError> {
        for (path, stamp) in &self.guards {
            stamp.verify(path)?;
        }
        Ok(())
    }
}
#[derive(Default)]
pub(crate) struct Cache {
    entries: VecDeque<(String, Instant, Arc<Prepared>)>,
}
impl Cache {
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
    pub(super) fn get(&mut self, key: &str) -> Result<Option<Arc<Prepared>>, ApiError> {
        self.entries
            .retain(|(_, used, _)| used.elapsed() < Duration::from_secs(30));
        let Some(index) = self.entries.iter().position(|(k, _, _)| k == key) else {
            return Ok(None);
        };
        let (key, _, entry) = self.entries.remove(index).unwrap();
        // Failed witnesses evict the entry. Do not return stale pixels or silently
        // fall back from a requested proxy to an original.
        entry.verify()?;
        self.entries.push_back((key, Instant::now(), entry.clone()));
        Ok(Some(entry))
    }
    pub(super) fn insert(&mut self, key: String, entry: Arc<Prepared>) {
        self.insert_with_limits(key, entry, 3, 1024 * 1024 * 1024);
    }
    fn insert_with_limits(&mut self, key: String, entry: Arc<Prepared>, count: usize, limit: u64) {
        let bytes = entry.video.probe().byte_length;
        if bytes > limit || count == 0 {
            return;
        }
        self.entries.retain(|(k, _, _)| k != &key);
        while self.entries.len() >= count
            || self
                .entries
                .iter()
                .map(|(_, _, p)| p.video.probe().byte_length)
                .sum::<u64>()
                + bytes
                > limit
        {
            self.entries.pop_front();
        }
        self.entries.push_back((key, Instant::now(), entry));
    }
}

#[cfg(test)]
#[path = "video_preview_cache_tests.rs"]
mod tests;
