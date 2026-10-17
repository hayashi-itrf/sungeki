use crate::{MAX_WAV_BYTES, WavError};
use std::ops::Range;

pub(crate) struct Chunks {
    pub format: Range<usize>,
    pub data: Range<usize>,
}

/// Hound decodes the first data chunk, but does not enforce the entire RIFF
/// envelope. Validate chunk extents and unique fmt/data before decoding.
pub(crate) fn validate(bytes: &[u8]) -> Result<Chunks, WavError> {
    if bytes.len() > MAX_WAV_BYTES {
        return Err(WavError::TooLarge);
    }
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(WavError::Container);
    }
    if u32_at(bytes, 4) as u64 + 8 != bytes.len() as u64 {
        return Err(WavError::Container);
    }
    let mut at = 12;
    let mut format = None;
    let mut data = None;
    let mut count = 0;
    while at < bytes.len() {
        count += 1;
        if count > 4096 || bytes.len() - at < 8 {
            return Err(WavError::Container);
        }
        let id = &bytes[at..at + 4];
        let size = u32_at(bytes, at + 4) as usize;
        let start = at + 8;
        if size > bytes.len() - start {
            return Err(WavError::Container);
        }
        let end = start + size;
        if id == b"fmt " {
            if format.is_some() || data.is_some() || size < 16 {
                return Err(WavError::Container);
            }
            let channels = u16_at(bytes, start + 2);
            let align = u16_at(bytes, start + 12);
            if channels == 0 || align == 0 || align % channels != 0 {
                return Err(WavError::Container);
            }
            format = Some(at..end);
        } else if id == b"data" {
            if data.is_some() || format.is_none() {
                return Err(WavError::Container);
            }
            if size == 0 {
                return Err(WavError::Empty);
            }
            data = Some(at..end);
        }
        at = end + (size & 1);
        if at > bytes.len() {
            return Err(WavError::Container);
        }
    }
    Ok(Chunks {
        format: format.ok_or(WavError::Container)?,
        data: data.ok_or(WavError::Container)?,
    })
}
fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().expect("bounded fmt"))
}
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("bounded header"))
}

/// Canonical decoding stream; excludes ancillary chunks Hound cannot pad.
pub(crate) fn reader(bytes: &[u8]) -> Result<hound::WavReader<impl std::io::Read + '_>, WavError> {
    use std::io::{Cursor, Read};
    let chunks = validate(bytes)?;
    let input = Cursor::new(&bytes[..12])
        .chain(Cursor::new(&bytes[chunks.format]))
        .chain(Cursor::new(&bytes[chunks.data]));
    hound::WavReader::new(input).map_err(|error| WavError::Decode(error.to_string()))
}
