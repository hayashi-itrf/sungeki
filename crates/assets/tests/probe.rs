use image::{DynamicImage, ImageFormat, RgbImage, RgbaImage};
use std::{fs, io::Cursor, path::PathBuf};
use sungeki_assets::{ProbeError, ProbeLimits, probe_image};
use sungeki_project::ImageFormat as ProbeFormat;
struct Fixture(PathBuf);
impl Fixture {
    fn new(bytes: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!("vvm-image-{}.bin", uuid::Uuid::new_v4()));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn encode(format: ImageFormat) -> Vec<u8> {
    let image = if format == ImageFormat::Jpeg {
        DynamicImage::ImageRgb8(RgbImage::new(8, 5))
    } else {
        DynamicImage::ImageRgba8(RgbaImage::new(8, 5))
    };
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, format).unwrap();
    bytes.into_inner()
}
#[test]
fn decodes_real_images_by_content_with_stable_hash_and_alpha() {
    for (format, name) in [
        (ImageFormat::Png, ProbeFormat::Png),
        (ImageFormat::Jpeg, ProbeFormat::Jpeg),
        (ImageFormat::WebP, ProbeFormat::Webp),
    ] {
        let file = Fixture::new(&encode(format));
        let probe = probe_image(&file.0, ProbeLimits::default()).unwrap();
        assert_eq!(
            (probe.width, probe.height, probe.format.clone()),
            (8, 5, name)
        );
        assert_eq!(probe.orientation, 1);
        assert_eq!(probe.has_alpha, format != ImageFormat::Jpeg);
        assert_eq!(probe.sha256.len(), 64);
        assert_eq!(probe, probe_image(&file.0, ProbeLimits::default()).unwrap());
    }
}
#[test]
fn rejects_invalid_and_truncated_content_not_just_bad_extension() {
    let file = Fixture::new(b"not an image");
    assert!(matches!(
        probe_image(&file.0, ProbeLimits::default()),
        Err(ProbeError::UnsupportedFormat)
    ));
    let mut png = encode(ImageFormat::Png);
    png.truncate(png.len() / 2);
    fs::write(&file.0, png).unwrap();
    assert!(probe_image(&file.0, ProbeLimits::default()).is_err());
}
#[test]
fn enforces_file_dimensions_pixels_and_decoded_size() {
    let file = Fixture::new(&encode(ImageFormat::Png));
    let default = ProbeLimits::default();
    for limits in [
        ProbeLimits {
            max_file_bytes: 4,
            ..default
        },
        ProbeLimits {
            max_dimension: 4,
            ..default
        },
        ProbeLimits {
            max_pixels: 39,
            ..default
        },
        ProbeLimits {
            max_decoded_bytes: 159,
            ..default
        },
    ] {
        assert!(matches!(
            probe_image(&file.0, limits),
            Err(ProbeError::TooLarge)
        ));
    }
}
#[test]
fn missing_directory_and_relative_inputs_are_not_images() {
    let dir = std::env::temp_dir();
    assert!(matches!(
        probe_image(&dir, ProbeLimits::default()),
        Err(ProbeError::NotAFile)
    ));
    assert!(matches!(
        probe_image(
            &dir.join(uuid::Uuid::new_v4().to_string()),
            ProbeLimits::default()
        ),
        Err(ProbeError::Io(_))
    ));
    assert!(matches!(
        probe_image(std::path::Path::new("image.png"), ProbeLimits::default()),
        Err(ProbeError::Io(_))
    ));
}
#[test]
fn replacing_content_changes_hash() {
    let file = Fixture::new(&encode(ImageFormat::Png));
    let first = probe_image(&file.0, ProbeLimits::default()).unwrap();
    fs::write(&file.0, encode(ImageFormat::WebP)).unwrap();
    let second = probe_image(&file.0, ProbeLimits::default()).unwrap();
    assert_ne!(first.sha256, second.sha256);
    assert_eq!(second.format, ProbeFormat::Webp);
}

#[test]
fn preserves_jpeg_orientation_instead_of_silently_swapping_dimensions() {
    let jpeg = encode(ImageFormat::Jpeg);
    // APP1 Exif: little-endian TIFF with one SHORT orientation entry (rotate 90 CW).
    let exif: &[u8] = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let mut bytes = vec![0xff, 0xd8, 0xff, 0xe1];
    bytes.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    bytes.extend_from_slice(exif);
    bytes.extend_from_slice(&jpeg[2..]);
    let file = Fixture::new(&bytes);
    let probe = probe_image(&file.0, ProbeLimits::default()).unwrap();
    assert_eq!(probe.orientation, 6);
    let (_, width, height) = sungeki_assets::image_preview(&file.0, &probe.sha256).unwrap();
    assert_eq!((width, height), (5, 8));
    assert_eq!((probe.width, probe.height), (8, 5));
}

#[test]
fn animated_png_is_not_silently_imported_as_a_still_image() {
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut crc = !0u32;
        for byte in &out[4..] {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb88320u32 & 0u32.wrapping_sub(crc & 1));
            }
        }
        out.extend_from_slice(&(!crc).to_be_bytes());
        out
    }
    let png = encode(ImageFormat::Png);
    let mut bytes = png[..33].to_vec(); // Signature and IHDR.
    bytes.extend(chunk(b"acTL", &[0, 0, 0, 1, 0, 0, 0, 0]));
    bytes.extend(chunk(
        b"fcTL",
        &[
            0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 10, 0, 0,
        ],
    ));
    bytes.extend_from_slice(&png[33..]);
    let file = Fixture::new(&bytes);
    assert!(matches!(
        probe_image(&file.0, ProbeLimits::default()),
        Err(ProbeError::AnimatedImage)
    ));
}

#[test]
fn preview_keeps_alpha_and_refuses_a_different_source_hash() {
    let file = Fixture::new(&encode(ImageFormat::Png));
    let probe = probe_image(&file.0, ProbeLimits::default()).unwrap();
    let (png, width, height) = sungeki_assets::image_preview(&file.0, &probe.sha256).unwrap();
    assert_eq!((width, height), (8, 5));
    let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(decoded.get_pixel(0, 0).0[3], 0);
    assert!(matches!(
        sungeki_assets::image_preview(&file.0, &"0".repeat(64)),
        Err(ProbeError::SourceMismatch)
    ));
}

#[test]
fn preview_bounds_large_images_without_changing_the_source() {
    let mut bytes = Cursor::new(Vec::new());
    RgbaImage::new(2048, 512)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    let file = Fixture::new(bytes.get_ref());
    let probe = probe_image(&file.0, ProbeLimits::default()).unwrap();
    let (_, width, height) = sungeki_assets::image_preview(&file.0, &probe.sha256).unwrap();
    assert_eq!((width, height), (1024, 256));
    assert_eq!(fs::read(&file.0).unwrap(), *bytes.get_ref());
}
