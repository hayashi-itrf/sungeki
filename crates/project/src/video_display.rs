//! Pure original-video display geometry shared by placement and rasterization.
use crate::VideoStreamInfo;
pub struct VideoDisplayGeometry {
    pub width: u32,
    pub height: u32,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    min_x: f64,
    min_y: f64,
    determinant: f64,
}
impl VideoDisplayGeometry {
    pub fn new(s: &VideoStreamInfo) -> Result<Self, &'static str> {
        let [a, b, c, d] = matrix(s)?;
        let sar = s
            .sample_aspect_ratio
            .map_or(1.0, |r| r.numerator as f64 / r.denominator as f64);
        let (a, b) = (a * sar, b * sar);
        let determinant = a * d - b * c;
        if !determinant.is_finite() || determinant.abs() < 1e-9 || s.width == 0 || s.height == 0 {
            return Err("singular display geometry");
        }
        let w = s.width as f64;
        let h = s.height as f64;
        let xs = [0.0, a * w, c * h, a * w + c * h];
        let ys = [0.0, b * w, d * h, b * w + d * h];
        let min_x = xs.into_iter().fold(f64::INFINITY, f64::min);
        let min_y = ys.into_iter().fold(f64::INFINITY, f64::min);
        let width = (xs.into_iter().fold(f64::NEG_INFINITY, f64::max) - min_x).ceil();
        let height = (ys.into_iter().fold(f64::NEG_INFINITY, f64::max) - min_y).ceil();
        if !width.is_finite()
            || !height.is_finite()
            || width < 1.0
            || height < 1.0
            || width > 16384.0
            || height > 16384.0
            || width * height * 4.0 > 64.0 * 1024.0 * 1024.0
        {
            return Err("display frame exceeds limits");
        }
        Ok(Self {
            width: width as u32,
            height: height as u32,
            a,
            b,
            c,
            d,
            min_x,
            min_y,
            determinant,
        })
    }
    pub fn source(&self, x: u32, y: u32) -> (f64, f64) {
        let x = x as f64 + 0.5 + self.min_x;
        let y = y as f64 + 0.5 + self.min_y;
        (
            (self.d * x - self.c * y) / self.determinant,
            (-self.b * x + self.a * y) / self.determinant,
        )
    }
}
fn matrix(s: &VideoStreamInfo) -> Result<[f64; 4], &'static str> {
    if let Some(raw) = &s.display_matrix {
        let mut values = Vec::new();
        for row in raw.lines().filter(|l| !l.trim().is_empty()) {
            let (_, numbers) = row.split_once(':').ok_or("display matrix row")?;
            let row: Vec<i32> = numbers
                .split_whitespace()
                .map(str::parse)
                .collect::<Result<_, _>>()
                .map_err(|_| "display matrix number")?;
            if row.len() != 3 {
                return Err("display matrix columns");
            }
            values.extend(row);
        }
        if values.len() != 9
            || values[2] != 0
            || values[5] != 0
            || values[8] != 1 << 30
            || values[6] != 0
            || values[7] != 0
        {
            // Translation requires a container canvas, which is not in VideoProbe.
            return Err("unsupported perspective/translated display matrix");
        }
        return Ok([values[0], values[1], values[3], values[4]].map(|n| n as f64 / 65536.0));
    }
    let angle = s
        .rotation_degrees
        .or_else(|| s.legacy_rotate_tag.as_ref().and_then(|s| s.parse().ok()));
    if s.legacy_rotate_tag.is_some() && angle.is_none() {
        return Err("invalid rotate tag");
    }
    let angle = angle.unwrap_or(0.0);
    if !angle.is_finite() {
        return Err("invalid rotation");
    }
    let (sin, cos) = angle.to_radians().sin_cos();
    // ffprobe reports counterclockwise rotation; raster y increases downwards.
    let snap = |v: f64| if v.abs() < 1e-12 { 0.0 } else { v };
    Ok([snap(cos), snap(-sin), snap(sin), snap(cos)])
}
