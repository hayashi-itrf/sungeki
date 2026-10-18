use crate::{Result, composite::over, wrapping};
use freetype::{Library, RenderMode, StrokerLineCap, StrokerLineJoin, face::LoadFlag};
use image::{Rgba, RgbaImage};
use serde_json::Value;
use std::collections::HashMap;
struct Raster {
    pixels: RgbaImage,
    left: i32,
    top: i32,
}
struct Glyph {
    ink: Raster,
    stroke: Option<Raster>,
    advance: f64,
}
pub(crate) struct Text {
    glyphs: HashMap<char, Glyph>,
    stroke: u32,
    size: f64,
}
fn raster(g: freetype::BitmapGlyph, color: [u8; 4]) -> Result<Raster> {
    let b = g.bitmap();
    if b.pitch() < 0 {
        return Err("Negative glyph pitch".into());
    }
    let mut pixels = RgbaImage::new(b.width() as u32, b.rows() as u32);
    for (x, y, p) in pixels.enumerate_pixels_mut() {
        *p = Rgba([
            color[0],
            color[1],
            color[2],
            b.buffer()[y as usize * b.pitch() as usize + x as usize],
        ]);
    }
    Ok(Raster {
        pixels,
        left: g.left(),
        top: g.top(),
    })
}
impl Text {
    pub(crate) fn new(font: &[u8], p: &Value) -> Result<Self> {
        let lib = Library::init()?;
        let face = lib
            .new_memory_face(font.to_vec(), 0)
            .map_err(|error| format!("字幕フォントの読込に失敗しました: {error}"))?;
        let size = p["font_size"].as_f64().ok_or("Invalid font size")?;
        face.set_char_size((size * 64.).round() as isize, 0, 72, 72)?;
        let stroke = p["outline"]["width"].as_u64().unwrap_or(0) as u32;
        let color = crate::frame::color(p["color"].as_str().ok_or("Missing text color")?)?;
        let border = crate::frame::color(p["outline"]["color"].as_str().unwrap_or("#000000"))?;
        let stroker = lib.new_stroker()?;
        stroker.set(
            stroke as i64 * 64,
            StrokerLineCap::Round,
            StrokerLineJoin::Round,
            0,
        );
        let mut glyphs = HashMap::new();
        for c in p["text"]
            .as_str()
            .ok_or("Missing text")?
            .chars()
            .filter(|c| *c != '\n' && *c != '\r')
        {
            if glyphs.contains_key(&c) {
                continue;
            }
            face.load_char(c as usize, LoadFlag::DEFAULT)?;
            let g = face.glyph().get_glyph()?;
            let ink = raster(g.to_bitmap(RenderMode::Normal, None)?, color)?;
            let outline = if stroke > 0 {
                Some(raster(
                    g.stroke(&stroker)?.to_bitmap(RenderMode::Normal, None)?,
                    border,
                )?)
            } else {
                None
            };
            glyphs.insert(
                c,
                Glyph {
                    ink,
                    stroke: outline,
                    advance: face.glyph().advance().x as f64 / 64.,
                },
            );
        }
        Ok(Self {
            glyphs,
            stroke,
            size,
        })
    }
    fn bounds(&self, s: &str) -> [f64; 4] {
        let (mut x, mut left, mut top, mut right, mut bottom) = (0f64, 0f64, 0f64, 0f64, 0f64);
        for c in s.chars() {
            if let Some(g) = self.glyphs.get(&c) {
                let r = g.stroke.as_ref().unwrap_or(&g.ink);
                left = left.min(x + r.left as f64);
                right = right.max(x + r.left as f64 + r.pixels.width() as f64);
                top = top.min(-r.top as f64);
                bottom = bottom.max(-r.top as f64 + r.pixels.height() as f64);
                x += g.advance;
            }
        }
        [left, top, right.max(x + self.stroke as f64), bottom]
    }
    pub(crate) fn draw(&self, dst: &mut RgbaImage, p: &Value, t: &Value) -> Result<Vec<[f64; 4]>> {
        let x = t["x"].as_f64().ok_or("x")?;
        let y = t["y"].as_f64().ok_or("y")?;
        let width = t["width"].as_f64().ok_or("width")?;
        let rows = wrapping::lines(
            p["text"].as_str().ok_or("text")?,
            width,
            p["layout"]["wrap"].as_bool().unwrap_or(false),
            |s| {
                let b = self.bounds(s);
                b[2] - b[0]
            },
        );
        let mut boxes = vec![];
        for (i, row) in rows.iter().enumerate() {
            if row.is_empty() {
                continue;
            }
            let b = self.bounds(row);
            let available = width - (b[2] - b[0]);
            let shift = match p["layout"]["alignment"].as_str() {
                Some("center") => available / 2.,
                Some("right") => available,
                _ => 0.,
            };
            let baseline = y + self.stroke as f64 + i as f64 * self.size * 1.3 - b[1];
            let origin = x + shift - b[0];
            for stroke in [true, false] {
                let mut pen = origin;
                for c in row.chars() {
                    if let Some(g) = self.glyphs.get(&c) {
                        let r = if stroke {
                            g.stroke.as_ref()
                        } else {
                            Some(&g.ink)
                        };
                        if let Some(r) = r {
                            over(
                                dst,
                                &r.pixels,
                                (pen + r.left as f64).round() as i64,
                                (baseline - r.top as f64).round() as i64,
                                1.,
                                false,
                            );
                        }
                        pen += g.advance;
                    }
                }
            }
            boxes.push([
                origin + b[0],
                baseline + b[1],
                origin + b[2],
                baseline + b[3],
            ]);
        }
        Ok(boxes)
    }
}
