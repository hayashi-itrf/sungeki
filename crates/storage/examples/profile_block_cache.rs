//! Prototype comparison on immutable snapshots; never writes a recovery database.
#[path = "support/block_cache.rs"]
mod block_cache;
#[allow(dead_code)]
#[path = "../src/snapshot_codec.rs"]
mod codec;
use std::time::Instant;
use sungeki_storage::StorageError;
fn main() {
    let mut cache = block_cache::Cache::default();
    for file in std::env::args().skip(1) {
        let raw = std::fs::read(&file).unwrap();
        let began = Instant::now();
        let whole = codec::encode(&raw).unwrap();
        let whole_ms = began.elapsed().as_secs_f64() * 1000.;
        let began = Instant::now();
        let blocks = cache.encode(&raw).unwrap();
        let block_ms = began.elapsed().as_secs_f64() * 1000.;
        let began = Instant::now();
        assert_eq!(blocks.decode().unwrap(), raw);
        let decode_ms = began.elapsed().as_secs_f64() * 1000.;
        assert_eq!(codec::decode(&whole).unwrap(), raw);
        println!(
            "{}",
            serde_json::json!({"file":file,"raw_bytes":raw.len(),
            "whole_encode_ms":whole_ms,"block_encode_ms":block_ms,"block_decode_ms":decode_ms,
            "whole_bytes":whole.len(),"block_bytes":blocks.bytes(),"cache_payload_bytes":cache.bytes(),
            "hit_blocks":blocks.hits,"roundtrip_equal":true})
        );
    }
}
