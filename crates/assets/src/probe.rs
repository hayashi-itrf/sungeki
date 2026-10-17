use image::{ImageDecoder, ImageFormat, ImageReader, Limits};
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::{Cursor, Read},
    path::Path,
};

use sungeki_project::ImageFormat as ProbeFormat;
pub use sungeki_project::ImageProbe;
#[derive(Debug, Clone, Copy)]
pub struct ProbeLimits {
    pub max_file_bytes: u64,
    pub max_dimension: u32,
    pub max_pixels: u64,
    pub max_decoded_bytes: u64,
}
impl Default for ProbeLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 64 * 1024 * 1024,
            max_dimension: 16_384,
            max_pixels: 64_000_000,
            max_decoded_bytes: 256 * 1024 * 1024,
        }
    }
}
#[derive(Debug)]
pub enum ProbeError {
    Io(std::io::Error),
    NotAFile,
    TooLarge,
    UnsupportedFormat,
    AnimatedImage,
    InvalidImage(image::ImageError),
    SourceChanged,
    SourceMismatch,
}
impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "素材を読めません: {e}"),
            Self::NotAFile => write!(f, "通常の画像ファイルを指定してください"),
            Self::TooLarge => write!(f, "画像の容量・寸法が検査上限を超えています"),
            Self::UnsupportedFormat => write!(f, "現在の画像検査はPNG・JPEG・WebPに対応しています"),
            Self::AnimatedImage => write!(f, "アニメーション画像は静止画として登録できません"),
            Self::InvalidImage(e) => write!(f, "画像をデコードできません: {e}"),
            Self::SourceMismatch => write!(
                f,
                "素材が登録時の内容から変わっています。素材を再指定してください"
            ),
            Self::SourceChanged => write!(f, "検査中に素材が変更されました。再検査してください"),
        }
    }
}
impl std::error::Error for ProbeError {}
impl From<std::io::Error> for ProbeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<image::ImageError> for ProbeError {
    fn from(e: image::ImageError) -> Self {
        match e {
            image::ImageError::Limits(_) => Self::TooLarge,
            _ => Self::InvalidImage(e),
        }
    }
}

/// Hash and decode the same captured bytes, never infer format from a filename.
/// This is inspection, not the immutable source snapshot required by render jobs.
pub fn probe_image(path: &Path, limit: ProbeLimits) -> Result<ImageProbe, ProbeError> {
    let bytes = read_image(path, limit)?;
    decode_image(&bytes, limit).map(|(probe, _)| probe)
}
/// Inspect original bytes without decoding pixels or creating a derivative.
pub fn image_source_hash(path: &Path, limit: ProbeLimits) -> Result<(String, u64), ProbeError> {
    let bytes = read_image(path, limit)?;
    Ok((format!("{:x}", Sha256::digest(&bytes)), bytes.len() as u64))
}
pub(crate) fn read_image(path: &Path, limit: ProbeLimits) -> Result<Vec<u8>, ProbeError> {
    if !path.is_absolute() {
        return Err(ProbeError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "検査には解決済みの絶対パスが必要です",
        )));
    }
    // Check before opening to avoid blocking on named pipes or other special files.
    if !path.metadata()?.is_file() {
        return Err(ProbeError::NotAFile);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    let before = file.metadata()?;
    if !before.is_file() {
        return Err(ProbeError::NotAFile);
    }
    if before.len() > limit.max_file_bytes {
        return Err(ProbeError::TooLarge);
    }
    let mut bytes = Vec::new();
    (&file)
        .take(limit.max_file_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit.max_file_bytes {
        return Err(ProbeError::TooLarge);
    }
    let after = file.metadata()?;
    if before.len() != after.len()
        || after.len() != bytes.len() as u64
        || before.modified().ok() != after.modified().ok()
    {
        return Err(ProbeError::SourceChanged);
    }
    Ok(bytes)
}
pub(crate) fn decode_image(
    bytes: &[u8],
    limit: ProbeLimits,
) -> Result<(ImageProbe, image::DynamicImage), ProbeError> {
    let format = image::guess_format(bytes).map_err(|_| ProbeError::UnsupportedFormat)?;
    let name = match format {
        ImageFormat::Png => ProbeFormat::Png,
        ImageFormat::Jpeg => ProbeFormat::Jpeg,
        ImageFormat::WebP => ProbeFormat::Webp,
        _ => return Err(ProbeError::UnsupportedFormat),
    };
    let mut limits = Limits::default();
    limits.max_image_width = Some(limit.max_dimension);
    limits.max_image_height = Some(limit.max_dimension);
    limits.max_alloc = Some(limit.max_decoded_bytes);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > limit.max_pixels
        || decoder.total_bytes() > limit.max_decoded_bytes
    {
        return Err(ProbeError::TooLarge);
    }
    let orientation = decoder.orientation()?.to_exif();
    let has_alpha = decoder.color_type().has_alpha();
    let animated = match format {
        ImageFormat::Png => image::codecs::png::PngDecoder::new(Cursor::new(bytes))?.is_apng()?,
        ImageFormat::WebP => {
            image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))?.has_animation()
        }
        _ => false,
    };
    if animated {
        return Err(ProbeError::AnimatedImage);
    }
    let decoded = image::DynamicImage::from_decoder(decoder)?;
    Ok((
        ImageProbe {
            format: name,
            width,
            height,
            orientation,
            has_alpha,
            byte_length: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        },
        decoded,
    ))
}
