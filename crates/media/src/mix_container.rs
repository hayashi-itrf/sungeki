//! Validate the complete RIFF envelope without allocating the mix in memory.
use crate::LoudnessError;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub(crate) fn validate(
    path: &Path,
    mut check: impl FnMut() -> Result<(), LoudnessError>,
) -> Result<(), LoudnessError> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let mut header = [0u8; 12];
    file.read_exact(&mut header)
        .map_err(|_| LoudnessError::InvalidReport)?;
    if &header[..4] != b"RIFF"
        || &header[8..] != b"WAVE"
        || u64::from(u32::from_le_bytes(header[4..8].try_into().unwrap())) + 8 != length
    {
        return Err(LoudnessError::InvalidReport);
    }
    let mut at = 12u64;
    let (mut format, mut data) = (false, false);
    let mut count = 0;
    while at < length {
        check()?;
        count += 1;
        if count > 4096 || length - at < 8 {
            return Err(LoudnessError::InvalidReport);
        }
        file.seek(SeekFrom::Start(at))?;
        let mut chunk = [0u8; 8];
        file.read_exact(&mut chunk)?;
        let size = u64::from(u32::from_le_bytes(chunk[4..].try_into().unwrap()));
        let end = at + 8 + size + (size & 1);
        if end > length {
            return Err(LoudnessError::InvalidReport);
        }
        match &chunk[..4] {
            b"fmt " => {
                if format || data || size < 16 {
                    return Err(LoudnessError::InvalidReport);
                }
                format = true;
            }
            b"data" => {
                // Every stereo float32 frame occupies exactly eight bytes.
                if data || !format || size == 0 || size % 8 != 0 {
                    return Err(LoudnessError::InvalidReport);
                }
                data = true;
            }
            _ => {}
        }
        at = end;
    }
    if !format || !data {
        return Err(LoudnessError::InvalidReport);
    }
    Ok(())
}
