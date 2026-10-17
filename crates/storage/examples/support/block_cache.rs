//! Experimental only: independent compressed blocks; not a persisted format.
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use sungeki_storage::StorageError;
#[allow(dead_code)]
#[path = "../../src/snapshot_codec.rs"]
mod codec;
const BLOCK: usize = 65536;
const MAX_RAW: usize = 128 * 1024 * 1024;
const MAX_CACHE: usize = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
#[derive(Default)]
pub struct Cache {
    entries: HashMap<[u8; 32], Vec<u8>>,
    order: VecDeque<[u8; 32]>,
    bytes: usize,
}
pub struct Encoded {
    len: usize,
    hash: [u8; 32],
    blocks: Vec<Vec<u8>>,
    pub hits: usize,
}
impl Encoded {
    pub fn bytes(&self) -> usize {
        48 + self.blocks.iter().map(|b| b.len() + 4).sum::<usize>()
    }
    pub fn decode(&self) -> Result<Vec<u8>, StorageError> {
        if self.len > MAX_RAW || self.blocks.len() != self.len.div_ceil(BLOCK) {
            return Err(StorageError::InvalidSnapshot);
        }
        let mut result = Vec::new();
        for (index, block) in self.blocks.iter().enumerate() {
            let expected = (self.len - index * BLOCK).min(BLOCK);
            if block.len() < 48
                || u64::from_le_bytes(block[8..16].try_into().unwrap()) != expected as u64
            {
                return Err(StorageError::InvalidSnapshot);
            }
            result.extend(codec::decode(block)?);
        }
        if result.len() != self.len || <[u8; 32]>::from(Sha256::digest(&result)) != self.hash {
            return Err(StorageError::InvalidSnapshot);
        }
        Ok(result)
    }
}
impl Cache {
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn encode(&mut self, raw: &[u8]) -> Result<Encoded, StorageError> {
        if raw.len() > MAX_RAW {
            return Err(StorageError::InvalidSnapshot);
        }
        let mut blocks = Vec::new();
        let mut hits = 0;
        for block in raw.chunks(BLOCK) {
            let key: [u8; 32] = Sha256::digest(block).into();
            if let Some(encoded) = self.entries.get(&key) {
                blocks.push(encoded.clone());
                hits += 1;
            } else {
                let encoded = codec::encode(block)?;
                let cost = encoded.len() + 32;
                while self.bytes + cost > MAX_CACHE || self.entries.len() >= MAX_ENTRIES {
                    let old = self.order.pop_front().unwrap();
                    self.bytes -= self.entries.remove(&old).unwrap().len() + 32;
                }
                self.bytes += cost;
                self.order.push_back(key);
                self.entries.insert(key, encoded.clone());
                blocks.push(encoded);
            }
        }
        Ok(Encoded {
            len: raw.len(),
            hash: Sha256::digest(raw).into(),
            blocks,
            hits,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_and_reuse_do_not_alias_returned_data() {
        let mut cache = Cache::default();
        let raw = vec![7; BLOCK * 2 + 3];
        let first = cache.encode(&raw).unwrap();
        assert_eq!(first.decode().unwrap(), raw);
        let mut second = cache.encode(&raw).unwrap();
        assert_eq!(second.hits, 3);
        second.blocks[0][16] ^= 1;
        assert!(second.decode().is_err());
        assert_eq!(cache.encode(&raw).unwrap().decode().unwrap(), raw);
        assert_eq!(cache.encode(&[]).unwrap().decode().unwrap(), b"");
    }
    #[test]
    fn rejects_reorder_truncation_trailing_bytes_and_oversized_headers() {
        let mut cache = Cache::default();
        let raw: Vec<_> = (0..BLOCK * 2).map(|n| (n / BLOCK) as u8).collect();
        let mut encoded = cache.encode(&raw).unwrap();
        encoded.blocks.swap(0, 1);
        assert!(encoded.decode().is_err());
        let mut encoded = cache.encode(&raw).unwrap();
        encoded.blocks.pop();
        assert!(encoded.decode().is_err());
        let mut encoded = cache.encode(&raw).unwrap();
        encoded.blocks[0].push(0);
        assert!(encoded.decode().is_err());
        let mut encoded = cache.encode(&raw).unwrap();
        encoded.blocks[0][8..16].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(encoded.decode().is_err());
    }
    #[test]
    fn incompressible_blocks_respect_the_payload_budget() {
        let mut cache = Cache::default();
        let mut seed = 17u64;
        for _ in 0..300 {
            let block: Vec<_> = (0..BLOCK)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    seed as u8
                })
                .collect();
            let encoded = cache.encode(&block).unwrap();
            assert_eq!(encoded.decode().unwrap(), block);
            assert!(cache.bytes <= MAX_CACHE);
        }
        assert!(cache.entries.len() < 300);
    }
    #[test]
    fn entry_limit_evicts_old_content_and_retains_roundtrip() {
        let mut cache = Cache::default();
        for n in 0u32..4200 {
            assert_eq!(
                cache.encode(&n.to_le_bytes()).unwrap().decode().unwrap(),
                n.to_le_bytes()
            );
        }
        assert!(cache.entries.len() <= MAX_ENTRIES && cache.bytes <= MAX_CACHE);
        assert_eq!(cache.encode(&0u32.to_le_bytes()).unwrap().hits, 0);
    }
}
