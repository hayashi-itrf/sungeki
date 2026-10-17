//! Sprite definitions reference registered images, never directory-relative paths.
use crate::{Asset, AssetProbe, ImageFormat, ValidationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpritePose {
    pub name: String,
    pub closed_asset_id: String,
    pub open_asset_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpriteSet {
    pub default_pose: String,
    pub poses: Vec<SpritePose>,
}

impl SpriteSet {
    /// Structural validation only. Actual files are checked by the media pipeline.
    pub fn validate(&self, assets: &[Asset]) -> Result<(), ValidationError> {
        let invalid = |message: &str| ValidationError::new(message.into());
        if self.poses.is_empty() || self.poses.len() > 256 {
            return Err(invalid("スプライトのポーズ数は1〜256個にしてください"));
        }
        let mut images = HashMap::new();
        for asset in assets {
            if images.insert(asset.id.as_str(), asset).is_some() {
                return Err(invalid("スプライトの参照素材IDが重複しています"));
            }
        }
        let mut names = HashSet::new();
        let mut dimensions = None;
        for pose in &self.poses {
            if pose.name.trim().is_empty()
                || pose.name.len() > 256
                || pose.name.chars().any(char::is_control)
                || !names.insert(pose.name.as_str())
            {
                return Err(invalid("スプライトのポーズ名が不正または重複しています"));
            }
            for id in std::iter::once(&pose.closed_asset_id).chain(pose.open_asset_id.iter()) {
                if id.trim().is_empty() || id.chars().any(char::is_control) {
                    return Err(invalid("スプライトの参照素材IDが不正です"));
                }
                let asset = images
                    .get(id.as_str())
                    .ok_or_else(|| invalid("スプライトの参照画像が登録されていません"))?;
                // Reuse the same probe/hash rules as ordinary registered images.
                crate::asset_validation::validate_assets(std::slice::from_ref(*asset))?;
                let AssetProbe::Image(probe) = &asset.probe else {
                    return Err(invalid("スプライトにはPNG画像を指定してください"));
                };
                if probe.format != ImageFormat::Png || probe.orientation != 1 {
                    return Err(invalid("スプライトには回転情報のないPNG画像が必要です"));
                }
                let size = (probe.width, probe.height);
                if dimensions.is_some_and(|expected| expected != size) {
                    return Err(invalid("スプライトの全差分画像は同じ寸法にしてください"));
                }
                dimensions = Some(size);
            }
        }
        self.pose(None)?;
        Ok(())
    }

    /// Only an omitted pose uses the default; misspelled explicit names are errors.
    pub fn pose(&self, requested: Option<&str>) -> Result<&SpritePose, ValidationError> {
        let name = requested.unwrap_or(&self.default_pose);
        let mut matches = self.poses.iter().filter(|pose| pose.name == name);
        let pose = matches.next().ok_or_else(|| {
            ValidationError::new(format!("スプライトのポーズがありません: {name}"))
        })?;
        if matches.next().is_some() {
            return Err(ValidationError::new(
                "スプライトのポーズ名が重複しています".into(),
            ));
        }
        Ok(pose)
    }

    /// Missing open-mouth artwork deliberately keeps the closed-mouth image.
    pub fn asset_id(&self, pose: Option<&str>, open: bool) -> Result<&str, ValidationError> {
        let pose = self.pose(pose)?;
        Ok(if open {
            pose.open_asset_id
                .as_deref()
                .unwrap_or(&pose.closed_asset_id)
        } else {
            &pose.closed_asset_id
        })
    }

    /// All variants are dependencies, including poses not currently displayed.
    pub fn asset_ids(&self) -> Vec<&str> {
        let mut seen = HashSet::new();
        self.poses
            .iter()
            .flat_map(|pose| {
                std::iter::once(pose.closed_asset_id.as_str()).chain(pose.open_asset_id.as_deref())
            })
            .filter(|id| seen.insert(*id))
            .collect()
    }
}
