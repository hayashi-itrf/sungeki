use crate::{
    Result,
    frame::{Rasterizer, verified},
};
use image::ImageEncoder;
use serde_json::Value;
use std::{path::Path, sync::atomic::AtomicBool};
pub fn render(
    raster: &mut Rasterizer,
    input: &Path,
    resources: &Path,
    font: &Path,
    hash: &str,
    output: &Path,
    flag: &AtomicBool,
) -> Result<Value> {
    let packet = serde_json::from_slice(&std::fs::read(input)?)?;
    let resources = serde_json::from_slice(&std::fs::read(resources)?)?;
    let font = verified(font, hash)?;
    let (image, report) = raster.render(&packet, &resources, &font, hash, flag)?;
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let stream = options.open(output)?;
    image::codecs::png::PngEncoder::new_with_quality(
        stream,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Adaptive,
    )
    .write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(report)
}
