//! Layer-derived PSD poses. Embedded composite/thumbnail images are never used.
use crate::{Result, composite::over};
use ag_psd::psd::{BlendMode, ColorMode, Layer, Psd, ReadOptions};
use image::RgbaImage;
use sha2::{Digest, Sha256};
use sungeki_project::{PsdLayer, PsdLayerKind, PsdPose, PsdProbe};

pub struct Document {
    psd: Psd,
    profile: Option<Vec<u8>>,
    pub probe: PsdProbe,
}
impl Document {
    pub fn read(bytes: &[u8], expected: &str) -> Result<Self> {
        if bytes.len() > 64 * 1024 * 1024 || format!("{:x}", Sha256::digest(bytes)) != expected {
            return Err("PSD original hash/size mismatch".into());
        }
        if bytes.len() < 26 || &bytes[..6] != b"8BPS\0\x01" {
            return Err("Invalid PSD header".into());
        }
        let height = u32::from_be_bytes(bytes[14..18].try_into()?);
        let width = u32::from_be_bytes(bytes[18..22].try_into()?);
        if width == 0
            || height == 0
            || width > 16384
            || height > 16384
            || u64::from(width) * u64::from(height) > 32 * 1024 * 1024
        {
            return Err("PSD canvas limit".into());
        }
        let psd = ag_psd::read_psd(
            bytes,
            &ReadOptions {
                skip_composite_image_data: Some(true),
                skip_thumbnail: Some(true),
                skip_linked_files_data: Some(true),
                use_image_data: Some(true),
                total_memory_limit: Some(1024 * 1024 * 1024),
                ..Default::default()
            },
        )?;
        if psd.color_mode != Some(ColorMode::Rgb) || psd.bits_per_channel != Some(8.) {
            return Err("Only 8-bit RGB PSD is supported".into());
        }
        let mut probe = PsdProbe {
            sha256: expected.into(),
            byte_length: bytes.len() as u64,
            width: psd.width as u32,
            height: psd.height as u32,
            layer_layout_version: 1,
            layers: vec![],
        };
        catalog(
            psd.children.as_deref().unwrap_or_default(),
            "",
            &mut probe.layers,
        )?;
        probe.validate()?;
        Ok(Self {
            profile: crate::color_profile::extract(bytes)?,
            psd,
            probe,
        })
    }
    pub fn render(&self, pose: &PsdPose) -> Result<RgbaImage> {
        if pose.source_sha256 != self.probe.sha256 {
            return Err("Pose belongs to another PSD".into());
        }
        // Common model validates paths, option membership, and selection count.
        pose.validate(&self.probe)?;
        let mut pixels = RgbaImage::new(self.probe.width, self.probe.height);
        paint(
            self.psd.children.as_deref().unwrap_or_default(),
            "",
            pose,
            &mut pixels,
        )?;
        if !pixels.pixels().any(|p| p[3] != 0) {
            return Err("Pose is completely transparent".into());
        }
        if let Some(profile) = &self.profile {
            crate::color_profile::apply(&mut pixels, profile)?;
        }
        Ok(pixels)
    }
}
fn name(layer: &Layer) -> &str {
    layer.additional_info.name.as_deref().unwrap_or("")
}
fn catalog(layers: &[Layer], prefix: &str, output: &mut Vec<PsdLayer>) -> Result<()> {
    if prefix.matches('/').count() > 32 {
        return Err("PSD nesting limit".into());
    }
    for (i, l) in layers.iter().enumerate() {
        let path = format!("{prefix}/{i}");
        output.push(PsdLayer {
            path: path.clone(),
            name: name(l).into(),
            visible: !l.hidden.unwrap_or(false),
            kind: if l.children.is_some() {
                PsdLayerKind::Group
            } else {
                PsdLayerKind::Pixel
            },
        });
        if output.len() > 4096 {
            return Err("PSD layer limit".into());
        }
        if let Some(children) = &l.children {
            catalog(children, &path, output)?;
        }
    }
    Ok(())
}
fn paint(layers: &[Layer], prefix: &str, pose: &PsdPose, dst: &mut RgbaImage) -> Result<()> {
    for (i, l) in layers.iter().enumerate() {
        let path = format!("{prefix}/{i}");
        let selected = pose.selections.iter().find(|s| s.group == prefix);
        let visible = if name(l).starts_with('*') {
            selected
                .map(|s| s.option == path)
                .unwrap_or(!l.hidden.unwrap_or(false))
        } else {
            !l.hidden.unwrap_or(false)
        };
        if !visible {
            continue;
        }
        let a = &l.additional_info;
        if a.text.is_some()
            || a.adjustment.is_some()
            || a.placed_layer.is_some()
            || a.vector_fill.is_some()
            || a.vector_mask.is_some()
            || a.effects.is_some()
            || a.mask.is_some()
            || l.clipping == Some(true)
        {
            return Err(format!("Unsupported visible PSD feature in {}", name(l)).into());
        }
        let opacity = l.opacity.unwrap_or(1.) * a.fill_opacity.unwrap_or(1.);
        let mode = l.blend_mode.unwrap_or(BlendMode::Normal);
        if !matches!(
            mode,
            BlendMode::Normal | BlendMode::Multiply | BlendMode::PassThrough
        ) {
            return Err(format!("Unsupported PSD blend mode: {mode:?}").into());
        }
        if let Some(children) = &l.children {
            if mode == BlendMode::PassThrough && opacity == 1. {
                paint(children, &path, pose, dst)?;
            } else {
                let mut group = RgbaImage::new(dst.width(), dst.height());
                paint(children, &path, pose, &mut group)?;
                over(dst, &group, 0, 0, opacity, mode == BlendMode::Multiply);
            }
        } else if let Some(data) = l.image_data.as_ref().or(l.canvas.as_ref()) {
            let src = RgbaImage::from_raw(data.width, data.height, data.data.clone())
                .ok_or("Invalid PSD pixels")?;
            over(
                dst,
                &src,
                l.left.unwrap_or(0.) as i64,
                l.top.unwrap_or(0.) as i64,
                opacity,
                mode == BlendMode::Multiply,
            );
        }
    }
    Ok(())
}
