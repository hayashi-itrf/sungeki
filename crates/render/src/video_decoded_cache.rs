//! Job-local original pixels, never a proxy or a persistent source identity.
use std::collections::{BTreeMap, VecDeque};
use sungeki_media::video_probe::frames::SourceFrame;
type Key = (String, usize);
const LIMIT: usize = 128 * 1024 * 1024;
#[derive(Default)]
pub(super) struct DecodedCache {
    frames: BTreeMap<Key, SourceFrame>,
    order: VecDeque<Key>,
    bytes: usize,
}
impl DecodedCache {
    pub fn get(&mut self, key: &Key) -> Option<SourceFrame> {
        let frame = self.frames.get(key)?.clone();
        self.order.retain(|k| k != key);
        self.order.push_back(key.clone());
        Some(frame)
    }
    pub fn insert(&mut self, key: Key, frame: SourceFrame) {
        self.insert_bounded(key, frame, LIMIT);
    }
    fn insert_bounded(&mut self, key: Key, frame: SourceFrame, limit: usize) {
        let size = frame.rgba.len();
        if size > limit || self.frames.contains_key(&key) {
            return;
        }
        while self.bytes + size > limit || self.frames.len() >= 256 {
            let oldest = self.order.pop_front().expect("accounted cache");
            self.bytes -= self.frames.remove(&oldest).unwrap().rgba.len();
        }
        self.bytes += size;
        self.order.push_back(key.clone());
        self.frames.insert(key, frame);
    }
}

/// Decode only a contiguous run of frames actually requested by this job.
/// Temporary batch pixels <=64MiB and <=16 frames (in addition to the cache).
pub(super) fn batch_range(
    start: usize,
    bytes: u64,
    requested: Option<&std::collections::BTreeSet<usize>>,
) -> std::ops::Range<usize> {
    let count = if bytes == 0 {
        1
    } else {
        (64 * 1024 * 1024 / bytes).clamp(1, 16) as usize
    };
    let mut end = start + 1;
    while end - start < count && requested.is_some_and(|set| set.contains(&end)) {
        end += 1;
    }
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batches_stop_at_unused_frames_byte_budget_and_count_limit() {
        let set = (0..50).filter(|i| *i != 3).collect();
        assert_eq!(batch_range(0, 4, Some(&set)), 0..3);
        assert_eq!(batch_range(4, 4, Some(&set)), 4..20);
        assert_eq!(batch_range(4, 32 * 1024 * 1024, Some(&set)), 4..6);
        assert_eq!(batch_range(4, 64 * 1024 * 1024, Some(&set)), 4..5);
        assert_eq!(batch_range(4, 4, None), 4..5);
    }
    fn frame() -> SourceFrame {
        let p: sungeki_project::Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json"))
                .unwrap();
        let sungeki_project::AssetProbe::Video(probe) = p.assets[0].probe.clone() else {
            panic!()
        };
        SourceFrame {
            rgba: vec![1, 2, 3, 4],
            stream: probe.metadata.video,
            frame_index: 0,
            pts: 0,
        }
    }
    #[test]
    fn byte_budget_eviction_is_lru_and_asset_identity_is_separate() {
        let mut c = DecodedCache::default();
        let a = ("a".into(), 0);
        let b = ("b".into(), 0);
        let d = ("a".into(), 1);
        c.insert_bounded(a.clone(), frame(), 8);
        c.insert_bounded(b.clone(), frame(), 8);
        let mut returned = c.get(&a).unwrap();
        returned.rgba[0] = 99;
        c.insert_bounded(d.clone(), frame(), 8);
        assert!(c.get(&b).is_none());
        assert_eq!(c.get(&a).unwrap().rgba[0], 1);
        assert!(c.get(&d).is_some());
        assert_eq!(c.bytes, 8);
        c.insert_bounded(("too-large".into(), 0), frame(), 3);
        assert_eq!(c.bytes, 8);
    }
    #[test]
    fn tiny_frames_also_have_an_entry_limit() {
        let mut c = DecodedCache::default();
        for i in 0..300 {
            c.insert(("a".into(), i), frame());
        }
        assert_eq!(c.frames.len(), 256);
        assert_eq!(c.bytes, 1024);
        assert!(c.get(&("a".into(), 0)).is_none());
        assert!(c.get(&("a".into(), 299)).is_some());
    }
}
