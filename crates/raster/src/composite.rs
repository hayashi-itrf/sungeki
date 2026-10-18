use image::RgbaImage;
/// Straight-alpha source over, optionally multiplying the source with backdrop.
pub(crate) fn over(
    dst: &mut RgbaImage,
    src: &RgbaImage,
    x: i64,
    y: i64,
    opacity: f64,
    multiply: bool,
) {
    for sy in 0..src.height() {
        let dy = sy as i64 + y;
        if dy < 0 || dy >= dst.height() as i64 {
            continue;
        }
        for sx in 0..src.width() {
            let dx = sx as i64 + x;
            if dx < 0 || dx >= dst.width() as i64 {
                continue;
            }
            let s = src.get_pixel(sx, sy).0;
            let a = s[3] as f64 / 255. * opacity;
            if a == 0. {
                continue;
            }
            let d = dst.get_pixel_mut(dx as u32, dy as u32);
            let b = d[3] as f64 / 255.;
            let out = a + b * (1. - a);
            for c in 0..3 {
                let blend = if multiply {
                    s[c] as f64 * d[c] as f64 / 255.
                } else {
                    s[c] as f64
                };
                let color = (1. - b) * s[c] as f64 + b * blend;
                d[c] = ((a * color + b * (1. - a) * d[c] as f64) / out)
                    .round()
                    .clamp(0., 255.) as u8;
            }
            d[3] = (out * 255.).round() as u8;
        }
    }
}
