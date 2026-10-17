//! Reuse verified decoding only when all encoded bytes read from SQLite match.
use crate::{StorageError, snapshot_codec};
use std::sync::Arc;

const MAX_BYTES: usize = 128 * 1024 * 1024;
#[derive(Default)]
pub(crate) struct SnapshotCache(Option<(Vec<u8>, Arc<Vec<u8>>)>);
impl SnapshotCache {
    pub(crate) fn decode(&mut self, encoded: Vec<u8>) -> Result<Arc<Vec<u8>>, StorageError> {
        if let Some((key, decoded)) = &self.0 {
            if *key == encoded {
                return Ok(decoded.clone());
            }
        }
        let decoded = Arc::new(snapshot_codec::decode(&encoded)?);
        self.remember(encoded, &decoded);
        Ok(decoded)
    }
    // Call only after successful decode, or after the encoded write commits.
    pub(crate) fn remember(&mut self, encoded: Vec<u8>, decoded: &Arc<Vec<u8>>) {
        self.0 = if encoded.len().saturating_add(decoded.len()) <= MAX_BYTES {
            Some((encoded, Arc::clone(decoded)))
        } else {
            None
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verified_bytes_are_reused_but_modified_bytes_are_rechecked() {
        let encoded = snapshot_codec::encode(b"verified").unwrap();
        let mut cache = SnapshotCache::default();
        let mut returned = cache.decode(encoded.clone()).unwrap();
        Arc::make_mut(&mut returned)[0] = b'X';
        assert_eq!(
            cache.decode(encoded.clone()).unwrap().as_slice(),
            b"verified"
        );
        let mut corrupt = encoded.clone();
        corrupt[16] ^= 1;
        assert!(matches!(
            cache.decode(corrupt),
            Err(StorageError::InvalidSnapshot)
        ));
        assert_eq!(cache.decode(encoded).unwrap().as_slice(), b"verified");
        let other = snapshot_codec::encode(b"updated").unwrap();
        assert_eq!(cache.decode(other).unwrap().as_slice(), b"updated");
    }
    #[test]
    fn combined_size_limit_evicts_the_previous_cached_value() {
        let mut cache = SnapshotCache::default();
        cache
            .decode(snapshot_codec::encode(b"small").unwrap())
            .unwrap();
        cache.remember(vec![0; MAX_BYTES], &Arc::new(b"over budget".to_vec()));
        assert!(cache.0.is_none());
    }
}
