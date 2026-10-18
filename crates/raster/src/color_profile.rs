//! PSD ICC resources are not exposed by the layer parser. Read only resource 1039.
use crate::Result;
use image::RgbaImage;
fn take<'a>(data: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    if n > data.len() {
        return Err("Truncated PSD resource".into());
    }
    let (a, b) = data.split_at(n);
    *data = b;
    Ok(a)
}
fn u32be(data: &mut &[u8]) -> Result<usize> {
    Ok(u32::from_be_bytes(take(data, 4)?.try_into()?) as usize)
}
pub(crate) fn extract(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    let mut data = bytes;
    take(&mut data, 26)?;
    let n = u32be(&mut data)?;
    take(&mut data, n)?;
    let n = u32be(&mut data)?;
    let mut resources = take(&mut data, n)?;
    while !resources.is_empty() {
        if take(&mut resources, 4)? != b"8BIM" {
            return Err("Invalid PSD resource signature".into());
        }
        let id = u16::from_be_bytes(take(&mut resources, 2)?.try_into()?);
        let name = take(&mut resources, 1)?[0] as usize;
        take(&mut resources, name + ((name + 1) % 2))?;
        let size = u32be(&mut resources)?;
        let value = take(&mut resources, size)?;
        if id == 1039 {
            return Ok(Some(value.to_vec()));
        }
        if size % 2 != 0 {
            take(&mut resources, 1)?;
        }
    }
    Ok(None)
}
pub(crate) fn apply(image: &mut RgbaImage, profile: &[u8]) -> Result<()> {
    let source = lcms2::Profile::new_icc(profile)?;
    let destination = lcms2::Profile::new_srgb();
    let transform = lcms2::Transform::<[u8; 3], [u8; 3]>::new(
        &source,
        lcms2::PixelFormat::RGB_8,
        &destination,
        lcms2::PixelFormat::RGB_8,
        lcms2::Intent::Perceptual,
    )?;
    let input: Vec<[u8; 3]> = image.pixels().map(|p| [p[0], p[1], p[2]]).collect();
    let mut output = vec![[0; 3]; input.len()];
    transform.transform_pixels(&input, &mut output);
    for (p, rgb) in image.pixels_mut().zip(output) {
        p.0[..3].copy_from_slice(&rgb);
    }
    Ok(())
}
