use crate::{
    ProbeError, ProbeLimits,
    probe::{decode_image, read_image},
};
use image::{ImageFormat, metadata::Orientation};
use sha2::{Digest, Sha256};
use std::{io::Cursor, path::Path};

/// Display derivative only. It must never replace the source in formal rendering.
pub fn image_preview(path: &Path, expected_hash: &str) -> Result<(Vec<u8>, u32, u32), ProbeError> {
    let limit = ProbeLimits::default();
    let bytes = read_image(path, limit)?;
    if format!("{:x}", Sha256::digest(&bytes)) != expected_hash {
        return Err(ProbeError::SourceMismatch);
    }
    let (probe, mut image) = decode_image(&bytes, limit)?;
    image.apply_orientation(
        Orientation::from_exif(probe.orientation).ok_or(ProbeError::UnsupportedFormat)?,
    );
    let small = if image.width() > 1024 || image.height() > 1024 {
        image.thumbnail(1024, 1024).to_rgba8()
    } else {
        image.to_rgba8()
    };
    let dimensions = small.dimensions();
    let mut png = Cursor::new(Vec::new());
    small.write_to(&mut png, ImageFormat::Png)?;
    Ok((png.into_inner(), dimensions.0, dimensions.1))
}
