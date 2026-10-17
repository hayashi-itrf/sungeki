//! Return a bounded range only after verifying the entire artifact on one descriptor.
use crate::{PublishError, publish_output::validate_fingerprint, verified_copy::verified_copy};
use std::{
    io::{self, Write},
    path::Path,
};

struct Range {
    start: u64,
    end: u64,
    position: u64,
    bytes: Vec<u8>,
}
impl Write for Range {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let from = self
            .start
            .saturating_sub(self.position)
            .min(bytes.len() as u64) as usize;
        let to = self
            .end
            .saturating_sub(self.position)
            .min(bytes.len() as u64) as usize;
        if from < to {
            self.bytes.extend_from_slice(&bytes[from..to]);
        }
        self.position += bytes.len() as u64;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// At most 256KiB is retained. Each read rechecks the whole artifact, including bytes
/// outside the requested range. Caller must enforce its artifact size limit/deadline.
pub fn read_verified_output(
    path: &Path,
    sha256: &str,
    byte_length: u64,
    offset: u64,
    length: u32,
    mut cancelled: impl FnMut() -> bool,
) -> Result<Vec<u8>, PublishError> {
    validate_fingerprint(sha256, byte_length)?;
    if length == 0 || length > 256 * 1024 || offset >= byte_length {
        return Err(PublishError::InvalidRange);
    }
    let end = offset.saturating_add(u64::from(length)).min(byte_length);
    let mut range = Range {
        start: offset,
        end,
        position: 0,
        bytes: Vec::with_capacity((end - offset) as usize),
    };
    verified_copy(path, sha256, byte_length, &mut range, &mut cancelled)?;
    Ok(range.bytes)
}
