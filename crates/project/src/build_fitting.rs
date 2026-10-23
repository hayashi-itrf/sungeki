use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum BuildFitMode {
    Stretch,
    Contain,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildVisualFit {
    pub mode: BuildFitMode,
    /// Fraction of leftover space before the fitted image: 0=start, .5=center, 1=end.
    pub anchor_x: f64,
    pub anchor_y: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildVisualFits {
    pub standing: BTreeMap<String, BuildVisualFit>,
    pub diagram: Option<BuildVisualFit>,
    pub opening_logo: Option<BuildVisualFit>,
    pub opening_standing: BTreeMap<String, BuildVisualFit>,
}

impl BuildVisualFit {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !self.anchor_x.is_finite()
            || !self.anchor_y.is_finite()
            || !(0. ..=1.).contains(&self.anchor_x)
            || !(0. ..=1.).contains(&self.anchor_y)
        {
            return Err(ValidationError::new(
                "配置枠の基準点は0〜1で指定してください".into(),
            ));
        }
        Ok(())
    }
}

impl BuildVisualFits {
    pub fn validate(
        &self,
        recipe: &BuildRecipe,
        p: &BuildPresentation,
    ) -> Result<(), ValidationError> {
        for (role, fit) in &self.standing {
            fit.validate()?;
            if p.slots
                .as_ref()
                .is_none_or(|s| !s.standing.contains_key(role))
            {
                return Err(ValidationError::new(
                    "収め方を指定する人物の配置枠がありません".into(),
                ));
            }
            let role = recipe
                .roles
                .iter()
                .find(|r| &r.role == role)
                .ok_or_else(|| ValidationError::new("配置枠の出演者が未登録です".into()))?;
            if matches!(role.standing, BuildStanding::Vrm { .. })
                && fit.mode == BuildFitMode::Contain
            {
                return Err(ValidationError::new("VRMの枠内フィットは未対応です".into()));
            }
        }
        if let Some(fit) = &self.diagram {
            fit.validate()?;
            if p.slots.as_ref().is_none_or(|s| s.diagram.is_none()) {
                return Err(ValidationError::new("説明図の配置枠がありません".into()));
            }
        }
        if let Some(fit) = &self.opening_logo {
            fit.validate()?;
            if p.opening.as_ref().is_none_or(|o| o.logo.is_none()) {
                return Err(ValidationError::new(
                    "タイトルコールのロゴがありません".into(),
                ));
            }
        }
        for (role, fit) in &self.opening_standing {
            fit.validate()?;
            if p.opening
                .as_ref()
                .is_none_or(|o| !o.performers.iter().any(|r| &r.role == role))
            {
                return Err(ValidationError::new(
                    "タイトルコールの出演者が未登録です".into(),
                ));
            }
        }
        Ok(())
    }
}

/// Pure geometry used before committing the common build transaction.
/// Transparent margins belong to the image/PSD canvas; poses do not move the frame.
pub fn fit_visual_transform(
    assets: &[Asset],
    content: &ClipContent,
    frame: &Transform,
    fit: Option<&BuildVisualFit>,
) -> Result<Transform, ValidationError> {
    let Some(fit) = fit else {
        return Ok(frame.clone());
    };
    fit.validate()?;
    if fit.mode == BuildFitMode::Stretch {
        return Ok(frame.clone());
    }
    let id = match content {
        ClipContent::Image { asset_id }
        | ClipContent::Psd { asset_id, .. }
        | ClipContent::Video { asset_id, .. } => asset_id.as_str(),
        ClipContent::Sprite { sprites, pose, .. } => sprites.asset_id(pose.as_deref(), false)?,
        _ => {
            return Err(ValidationError::new(
                "この素材の枠内フィットは未対応です".into(),
            ));
        }
    };
    let probe = &assets
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| ValidationError::new("配置する素材が見つかりません".into()))?
        .probe;
    let (w, h) = match probe {
        AssetProbe::Image(p) if (5..=8).contains(&p.orientation) => {
            (p.height as f64, p.width as f64)
        }
        AssetProbe::Image(p) => (p.width as f64, p.height as f64),
        AssetProbe::Psd(p) => (p.width as f64, p.height as f64),
        AssetProbe::Video(p) => {
            let g = VideoDisplayGeometry::new(&p.metadata.video)
                .map_err(|e| ValidationError::new(e.into()))?;
            (g.width as f64, g.height as f64)
        }
        _ => {
            return Err(ValidationError::new(
                "配置素材の寸法を取得できません".into(),
            ));
        }
    };
    if w <= 0. || h <= 0. {
        return Err(ValidationError::new("配置素材の寸法が不正です".into()));
    }
    let scale = (frame.width / w).min(frame.height / h);
    let mut result = frame.clone();
    result.width = w * scale;
    result.height = h * scale;
    result.x += (frame.width - result.width) * fit.anchor_x;
    result.y += (frame.height - result.height) * fit.anchor_y;
    Ok(result)
}
