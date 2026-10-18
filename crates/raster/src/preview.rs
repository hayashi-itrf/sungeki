use crate::{Result, frame::resize};
use image::{ImageEncoder, RgbaImage};
use serde_json::Value;
use std::path::Path;
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}
pub fn encode(image: RgbaImage) -> Result<Preview> {
    let ratio = (1024. / image.width() as f64)
        .min(1024. / image.height() as f64)
        .min(1.);
    let image = resize(
        &image,
        (image.width() as f64 * ratio).round().max(1.) as u32,
        (image.height() as f64 * ratio).round().max(1.) as u32,
    );
    let mut png = vec![];
    image::codecs::png::PngEncoder::new(&mut png).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    if png.len() > 5 * 1024 * 1024 {
        return Err("Preview output size limit".into());
    }
    Ok(Preview {
        width: image.width(),
        height: image.height(),
        png,
    })
}
pub fn video(resource: &Value) -> Result<Preview> {
    let image = if resource.is_null() {
        RgbaImage::new(1, 1)
    } else {
        let data = crate::frame::verified(
            Path::new(resource["path"].as_str().ok_or("Video path")?),
            resource["sha256"].as_str().ok_or("Video hash")?,
        )?;
        let meta = &resource["decoded_video"];
        let w = meta["width"]
            .as_u64()
            .filter(|w| *w <= 16384)
            .ok_or("Video width")? as u32;
        let h = meta["height"]
            .as_u64()
            .filter(|h| *h <= 16384)
            .ok_or("Video height")? as u32;
        RgbaImage::from_raw(w, h, data).ok_or("Video buffer size")?
    };
    encode(image)
}
