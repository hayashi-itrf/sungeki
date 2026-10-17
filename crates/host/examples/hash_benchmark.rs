//! Compare SHA-256 calculation with file I/O without changing engine originals.
use sha2::{Digest, Sha256};
use std::{io::Read, os::unix::fs::OpenOptionsExt};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started = std::time::Instant::now();
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    let input = std::env::args_os().nth(1);
    if let Some(path) = &input {
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err("regular file required".into());
        }
        let mut block = [0u8; 65536];
        loop {
            let n = file.read(&mut block)?;
            if n == 0 {
                break;
            }
            hash.update(&block[..n]);
            bytes += n as u64;
        }
    } else {
        let block = vec![42u8; 65536];
        for _ in 0..16384 {
            hash.update(std::hint::black_box(&block));
            bytes += block.len() as u64;
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    println!(
        "{}",
        serde_json::json!({"bytes":bytes,"seconds":seconds,"sha256":format!("{:x}",hash.finalize()),"arch":std::env::consts::ARCH,"file_io":input.is_some()})
    );
    Ok(())
}
