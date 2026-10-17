use crate::{Asset, AssetLocator, ValidationError};
use std::path::Path;

pub(crate) fn validate_assets(assets: &[Asset]) -> Result<(), ValidationError> {
    for asset in assets {
        // Relative bindings require history/checkpoint rebasing on Save As; do not
        // accept them until that transaction is connected. Keep source media in place.
        match &asset.locator {
            AssetLocator::Absolute { path }
                if !path.contains('\0') && Path::new(path).is_absolute() => {}
            AssetLocator::Absolute { .. } => {
                return Err(ValidationError::new("素材には絶対パスが必要です".into()));
            }
            _ => return Err(ValidationError::new(
                "相対参照・ライブラリ参照の素材登録はまだ未対応です。絶対パスを指定してください"
                    .into(),
            )),
        }
        if asset.content_hash.as_deref() != Some(asset.probe.sha256()) {
            return Err(ValidationError::new(
                "素材の内容ハッシュが不一致です".into(),
            ));
        }
        let crate::AssetProbe::Image(p) = &asset.probe else {
            if let crate::AssetProbe::Psd(p) = &asset.probe {
                p.validate()?;
            }
            if let crate::AssetProbe::Audio(p) = &asset.probe {
                p.validate()?;
            }
            if let crate::AssetProbe::Video(p) = &asset.probe {
                p.validate()?;
            }
            if let crate::AssetProbe::Vrm(p) = &asset.probe {
                p.validate()?;
            }
            continue;
        };
        let hash_valid = p.sha256.len() == 64
            && p.sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c));
        if p.width == 0
            || p.height == 0
            || p.byte_length == 0
            || p.byte_length > 9_007_199_254_740_991
            || !(1..=8).contains(&p.orientation)
            || !hash_valid
            || asset.content_hash.as_deref() != Some(p.sha256.as_str())
        {
            return Err(ValidationError::new(
                "素材の画像情報・内容ハッシュが不正です".into(),
            ));
        }
    }
    Ok(())
}
