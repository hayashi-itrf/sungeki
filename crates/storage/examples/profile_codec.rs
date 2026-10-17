//! Compare lossless recovery codecs against identical bytes without modifying any database.
use sha2::{Digest, Sha256};
#[allow(dead_code)]
#[path = "../src/snapshot_codec.rs"]
mod snapshot_codec;
use std::{
    io::{Read, Write},
    time::Instant,
};
use sungeki_storage::StorageError;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let input = if args.get(1).map(String::as_str) == Some("--recovery-db") {
        let db = rusqlite::Connection::open_with_flags(
            &args[2],
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let bytes: Vec<u8> = db
            .query_row("SELECT snapshot FROM sessions LIMIT 1", [], |r| r.get(0))
            .unwrap();
        snapshot_codec::decode(&bytes).unwrap()
    } else {
        std::fs::read(args.get(1).expect("snapshot path")).unwrap()
    };
    // Include the fastest encoder; measure its larger output rather than assuming
    // that the compression level is monotonic in latency for repetitive histories.
    for quality in [0, 1, 2, 3] {
        for _ in 0..3 {
            let began = Instant::now();
            let mut compressed = Vec::new();
            {
                let mut writer = brotli::CompressorWriter::new(&mut compressed, 65536, quality, 22);
                writer.write_all(&input).unwrap();
            }
            let encode = began.elapsed();
            let began = Instant::now();
            let mut decoded = Vec::new();
            brotli::Decompressor::new(compressed.as_slice(), 65536)
                .take(input.len() as u64 + 1)
                .read_to_end(&mut decoded)
                .unwrap();
            let decode = began.elapsed();
            let began = Instant::now();
            let hash = Sha256::digest(&decoded);
            let hash_time = began.elapsed();
            assert_eq!(input, decoded);
            std::hint::black_box(hash);
            println!(
                "{}",
                serde_json::json!({"quality":quality,"window_bits":22,
                "input_bytes":input.len(),"stored_bytes":compressed.len(),
                "encode_ms":encode.as_secs_f64()*1000.,"decode_ms":decode.as_secs_f64()*1000.,
                "hash_ms":hash_time.as_secs_f64()*1000., "sha256":format!("{hash:x}")})
            );
        }
    }
}
