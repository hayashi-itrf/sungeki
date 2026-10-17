use std::{collections::BTreeMap, path::Path};
use sungeki_project::{
    AssetLocator, AssetProbe, AudioRelocation, Project, ValidationError, validate,
};

/// Validated, simultaneous substitutions. Reapplying the same map is a no-op.
/// No filesystem I/O or claim of byte identity: the save host supplies that proof.
pub struct AudioRelocations(BTreeMap<String, Vec<AudioRelocation>>);
impl AudioRelocations {
    pub fn new(relocations: Vec<AudioRelocation>) -> Result<Self, ValidationError> {
        let mut by_id: BTreeMap<String, Vec<AudioRelocation>> = BTreeMap::new();
        for value in relocations {
            if uuid::Uuid::parse_str(&value.asset_id)
                .ok()
                .is_none_or(|id| id.to_string() != value.asset_id)
            {
                return Err(invalid("音声素材IDが不正です"));
            }
            absolute(&value.source)?;
            absolute(&value.destination)?;
            value.probe.validate()?;
            let list = by_id.entry(value.asset_id.clone()).or_default();
            if let Some(existing) = list
                .iter()
                .find(|r| r.source == value.source && r.probe == value.probe)
            {
                if existing.destination != value.destination {
                    return Err(invalid("同じ音声に異なる保存先が指定されています"));
                }
            } else {
                list.push(value);
            }
        }
        // Reject maps whose second application would move an already relocated
        // asset again. A destination->itself entry is harmless.
        for list in by_id.values() {
            for value in list {
                if list.iter().any(|next| {
                    next.source == value.destination
                        && next.probe == value.probe
                        && next.source != next.destination
                }) {
                    return Err(invalid("音声の保存先が別の移動元になっています"));
                }
            }
        }
        Ok(Self(by_id))
    }
    pub fn relocate_project(&self, project: &Project) -> Result<Project, ValidationError> {
        validate(project)?;
        let mut result = project.clone();
        for asset in &mut result.assets {
            let AssetProbe::Audio(probe) = &asset.probe else {
                continue;
            };
            if let Some(value) = self.0.get(&asset.id).and_then(|list| {
                list.iter()
                    .find(|r| r.source == asset.locator && &r.probe == probe)
            }) {
                asset.locator = value.destination.clone();
            }
        }
        validate(&result)?;
        Ok(result)
    }
}
fn absolute(locator: &AssetLocator) -> Result<(), ValidationError> {
    if matches!(locator, AssetLocator::Absolute { path } if !path.contains('\0') && Path::new(path).is_absolute())
    {
        Ok(())
    } else {
        Err(invalid("編集中の音声参照は解決済みの絶対パスが必要です"))
    }
}
fn invalid(message: &str) -> ValidationError {
    ValidationError::new(message.into())
}
