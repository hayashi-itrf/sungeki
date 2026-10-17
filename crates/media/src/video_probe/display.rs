//! Source display geometry. Matrix layout follows libavutil/display.h.
use super::{ProbeError, frames::SourceFrame};
use sungeki_project::VideoDisplayGeometry;
pub struct DisplayFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub pts: i64,
}
impl SourceFrame {
    /// Convert source pixels to square-pixel display coordinates. Reflections
    /// come from the full matrix, not its lossy rotation-angle projection.
    pub fn into_display(self) -> Result<DisplayFrame, ProbeError> {
        let g = VideoDisplayGeometry::new(&self.stream).map_err(ProbeError::Invalid)?;
        if self.rgba.len() as u64
            != u64::from(self.stream.width) * u64::from(self.stream.height) * 4
        {
            return Err(ProbeError::Invalid("source RGBA length"));
        }
        let mut rgba = vec![0; g.width as usize * g.height as usize * 4];
        for y in 0..g.height {
            for x in 0..g.width {
                let (sx, sy) = g.source(x, y);
                let pixel = sample(&self.rgba, self.stream.width, self.stream.height, sx, sy);
                let i = (y as usize * g.width as usize + x as usize) * 4;
                rgba[i..i + 4].copy_from_slice(&pixel);
            }
        }
        Ok(DisplayFrame {
            width: g.width,
            height: g.height,
            rgba,
            pts: self.pts,
        })
    }
}
// Bilinear interpolation in premultiplied alpha avoids dark/colored fringes.
fn sample(pixels: &[u8], w: u32, h: u32, x: f64, y: f64) -> [u8; 4] {
    if x < 0.0 || y < 0.0 || x >= w as f64 || y >= h as f64 {
        return [0; 4];
    }
    let x = (x - 0.5).clamp(0.0, w as f64 - 1.0);
    let y = (y - 0.5).clamp(0.0, h as f64 - 1.0);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let mut value = [0.0; 4];
    for (px, wx) in [(x0, 1.0 - x.fract()), ((x0 + 1).min(w - 1), x.fract())] {
        for (py, wy) in [(y0, 1.0 - y.fract()), ((y0 + 1).min(h - 1), y.fract())] {
            let i = (py as usize * w as usize + px as usize) * 4;
            let alpha = pixels[i + 3] as f64 / 255.0;
            for c in 0..3 {
                value[c] += pixels[i + c] as f64 * alpha * wx * wy;
            }
            value[3] += alpha * wx * wy;
        }
    }
    if value[3] <= 0.0 {
        return [0; 4];
    }
    [
        (value[0] / value[3]).round() as u8,
        (value[1] / value[3]).round() as u8,
        (value[2] / value[3]).round() as u8,
        (value[3] * 255.0).round() as u8,
    ]
}
#[cfg(test)]
#[path = "display_tests.rs"]
mod tests;
