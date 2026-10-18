//! Native ScenePlan rasterization; resources must be captured originals.
use crate::{Result, composite::over, text::Text};
use image::{Rgba, RgbaImage, imageops};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
pub const LAYOUT_VERSION: u32 = 10;
#[derive(Default)]
pub struct Rasterizer {
    images: HashMap<String, RgbaImage>,
    bytes: usize,
}
pub(crate) fn color(s: &str) -> Result<[u8; 4]> {
    if s.len() != 7 || !s.starts_with('#') || !s[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid color".into());
    }
    Ok([
        u8::from_str_radix(&s[1..3], 16)?,
        u8::from_str_radix(&s[3..5], 16)?,
        u8::from_str_radix(&s[5..7], 16)?,
        255,
    ])
}
pub fn verified(path: &Path, hash: &str) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    let meta = file.metadata()?;
    const LIMIT: u64 = 64 * 1024 * 1024;
    if !meta.is_file() || meta.len() > LIMIT {
        return Err("Invalid image source size/type".into());
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != meta.len() || format!("{:x}", Sha256::digest(&bytes)) != hash {
        return Err("Original content hash/size mismatch".into());
    }
    Ok(bytes)
}
fn number(v: &Value, key: &str, min: f64, max: f64) -> Result<f64> {
    let n = v[key].as_f64().ok_or_else(|| format!("Invalid {key}"))?;
    if !n.is_finite() || n < min || n > max {
        return Err(format!("Out-of-range {key}").into());
    }
    Ok(n)
}
impl Rasterizer {
    fn image(&mut self, p: &Value, resources: &Value) -> Result<RgbaImage> {
        let id = p["asset_id"].as_str().ok_or("Missing asset id")?;
        let r = &resources[id];
        let bytes = verified(
            Path::new(r["path"].as_str().ok_or("Missing asset path")?),
            r["sha256"].as_str().ok_or("Missing asset hash")?,
        )?;
        let key = serde_json::to_string(&(p, r))?;
        if let Some(image) = self.images.get(&key) {
            return Ok(image.clone());
        }
        let image = if p["type"] == "psd" {
            let doc = crate::psd::Document::read(&bytes, r["sha256"].as_str().unwrap())?;
            if serde_json::to_value(&doc.probe)? != r["probe"]["info"] {
                return Err("PSD catalog differs from original".into());
            }
            doc.render(&serde_json::from_value(p["pose"].clone())?)?
        } else if let Some(v) = r.get("decoded_video") {
            let w = number(v, "width", 1., 16384.)? as u32;
            let h = number(v, "height", 1., 16384.)? as u32;
            RgbaImage::from_raw(w, h, bytes).ok_or("Invalid decoded video size")?
        } else {
            let mut reader =
                image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(256 * 1024 * 1024);
            reader.limits(limits);
            let mut decoder = reader.into_decoder()?;
            use image::ImageDecoder;
            let orientation = decoder.orientation()?;
            let mut image = image::DynamicImage::from_decoder(decoder)?;
            image.apply_orientation(orientation);
            image.to_rgba8()
        };
        let size = image.len();
        if size + self.bytes <= 256 * 1024 * 1024 {
            self.bytes += size;
            self.images.insert(key, image.clone());
        }
        Ok(image)
    }
    pub fn render(
        &mut self,
        packet: &Value,
        resources: &Value,
        font: &[u8],
        hash: &str,
        flag: &AtomicBool,
    ) -> Result<(RgbaImage, Value)> {
        let c = &packet["canvas"];
        let w = number(c, "width", 1., 8192.)? as u32;
        let h = number(c, "height", 1., 8192.)? as u32;
        if w as u64 * h as u64 > 32 * 1024 * 1024 {
            return Err("Canvas pixel limit".into());
        }
        let mut result = RgbaImage::from_pixel(
            w,
            h,
            Rgba(color(c["background"].as_str().ok_or("Canvas color")?)?),
        );
        let layers = packet["layers"].as_array().ok_or("Missing layers")?;
        if layers.len() > 10000 {
            return Err("Layer limit".into());
        }
        let mut issues = vec![];
        for item in layers {
            if flag.load(Ordering::Relaxed) {
                return Err("Rasterization cancelled".into());
            }
            let t = &item["transform"];
            let p = &item["primitive"];
            let x = number(t, "x", -1e6, 1e6)?;
            let y = number(t, "y", -1e6, 1e6)?;
            let tw = number(t, "width", 0.000001, 32768.)?;
            let th = number(t, "height", 0.000001, 32768.)?;
            let alpha = number(t, "opacity", 0., 1.)?;
            let mut layer = RgbaImage::new(w, h);
            match p["type"].as_str().ok_or("Missing primitive")? {
                "image" | "psd" => {
                    let src = self.image(p, resources)?;
                    let (sw, sh) = (tw.round().max(1.) as u32, th.round().max(1.) as u32);
                    if sw as u64 * sh as u64 > 32 * 1024 * 1024 {
                        return Err("Transformed image limit".into());
                    }
                    let mut resized = resize(&src, sw, sh);
                    if t["flip_x"].as_bool() == Some(true) {
                        imageops::flip_horizontal_in_place(&mut resized);
                    }
                    over(
                        &mut layer,
                        &resized,
                        x.round() as i64,
                        y.round() as i64,
                        1.,
                        false,
                    );
                }
                "text" => {
                    let text = Text::new(font, p)?;
                    for b in text.draw(&mut layer, p, t)? {
                        let outside = b[0] < 0. || b[1] < 0. || b[2] > w as f64 || b[3] > h as f64;
                        if outside || b[0] < x || b[1] < y || b[2] > x + tw || b[3] > y + th {
                            issues.push(json!({"code":"TEXT_OVERFLOW","clip_id":item["clip_id"],"bounds":b,"outside_canvas":outside}));
                        }
                    }
                }
                "shape" => {
                    let rgba = Rgba(color(p["fill"].as_str().ok_or("Shape color")?)?);
                    let ellipse = match p["shape"].as_str() {
                        Some("rectangle") => false,
                        Some("ellipse") => true,
                        _ => return Err("Unsupported shape".into()),
                    };
                    let (left, top) = (x.round() as i64, y.round() as i64);
                    let (sw, sh) = (tw.round().max(1.) as i64, th.round().max(1.) as i64);
                    for py in top.max(0)..(top + sh).min(h as i64) {
                        for px in left.max(0)..(left + sw).min(w as i64) {
                            if !ellipse
                                || ((px as f64 - left as f64 + 0.5 - sw as f64 / 2.)
                                    / (sw as f64 / 2.))
                                    .powi(2)
                                    + ((py as f64 - top as f64 + 0.5 - sh as f64 / 2.)
                                        / (sh as f64 / 2.))
                                        .powi(2)
                                    <= 1.
                            {
                                layer.put_pixel(px as u32, py as u32, rgba);
                            }
                        }
                    }
                }
                "blur" => {
                    let sigma = number(p, "sigma", 0., 128.)? as f32;
                    let blurred = imageops::blur(&result, sigma);
                    for py in (y.round() as i64).max(0)..((y + th).round() as i64).min(h as i64) {
                        for px in (x.round() as i64).max(0)..((x + tw).round() as i64).min(w as i64)
                        {
                            layer.put_pixel(
                                px as u32,
                                py as u32,
                                *blurred.get_pixel(px as u32, py as u32),
                            );
                        }
                    }
                }
                _ => return Err("Unsupported primitive".into()),
            }
            over(&mut result, &layer, 0, 0, alpha, false);
        }
        Ok((
            result,
            json!({"frame_index":packet["frame_index"],"time_us":packet["time_us"],"font_sha256":hash,"layout_version":LAYOUT_VERSION,"issues":issues}),
        ))
    }
}
/// Resize premultiplied colors to avoid dark/white fringes at transparent edges.
pub fn resize(src: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    if src.dimensions() == (w, h) {
        return src.clone();
    }
    let mut premult = src.clone();
    for p in premult.pixels_mut() {
        for c in 0..3 {
            p[c] = (p[c] as f64 * p[3] as f64 / 255.).round() as u8;
        }
    }
    let mut output = imageops::resize(&premult, w, h, imageops::FilterType::Lanczos3);
    for p in output.pixels_mut() {
        if p[3] > 0 {
            for c in 0..3 {
                p[c] = (p[c] as f64 * 255. / p[3] as f64).round().min(255.) as u8;
            }
        }
    }
    output
}
