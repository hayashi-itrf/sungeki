//! Shared standing checks for production recipes and reusable character presets.
use crate::*;
fn require(ok: bool, message: &str) -> Result<(), ValidationError> {
    if ok {
        Ok(())
    } else {
        Err(ValidationError::new(message.into()))
    }
}
fn asset<'a>(project: &'a Project, id: &str) -> Result<&'a Asset, ValidationError> {
    let asset = project
        .assets
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| ValidationError::new("参照素材がありません".into()))?;
    require(
        !asset.credit.trim().is_empty()
            && asset.credit.len() <= 65536
            && !asset.credit.contains('\0'),
        "使用素材のクレジットが必要です",
    )?;
    Ok(asset)
}
fn image(project: &Project, id: &str) -> Result<(), ValidationError> {
    require(
        matches!(asset(project, id)?.probe, AssetProbe::Image(_)),
        "画像素材が必要です",
    )
}
impl BuildStanding {
    /// Project must first pass the shared structural validation.
    pub fn validate_character(
        &self,
        project: &Project,
        character_id: &str,
    ) -> Result<(), ValidationError> {
        require(
            project.characters.iter().any(|c| c.id == character_id),
            "話者が登録されていません",
        )?;
        match self {
            BuildStanding::Vrm { standing, .. } => {
                standing.validate(project, TimeUs(0))?;
                require(
                    standing.character_id.as_deref() == Some(character_id),
                    "VRMと話者が一致しません",
                )?;
                for id in [&standing.model_asset_id, &standing.motion_asset_id] {
                    asset(project, id)?;
                }
                let AssetProbe::Vrm(model) = &asset(project, &standing.model_asset_id)?.probe
                else {
                    unreachable!()
                };
                require(
                    model.inspection.expressions.iter().any(|n| n == "aa"),
                    "VRMの音声連動にはaa表情が必要です",
                )?;
            }
            BuildStanding::Sprite { sprites, pose, .. } => {
                sprites.validate(&project.assets)?;
                sprites.pose(pose.as_deref())?;
                for id in sprites.asset_ids() {
                    image(project, id)?;
                }
            }
            BuildStanding::Image { asset_id, .. } => {
                image(project, asset_id)?;
            }
            BuildStanding::Psd {
                asset_id,
                pose,
                speech,
                ..
            } => {
                let AssetProbe::Psd(probe) = &asset(project, asset_id)?.probe else {
                    return Err(ValidationError::new("立ち絵はPSD素材が必要です".into()));
                };
                require(
                    speech.character_id == character_id,
                    "立ち絵と話者が一致しません",
                )?;
                pose.validate(probe)?;
                speech.validate(pose, probe)?;
            }
        }
        Ok(())
    }
}
