//! Validated server metadata. This fingerprint is NOT a model-file content hash.
use super::{Error, Settings};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub config_path: String,
    pub model_path: String,
    pub device: String,
    pub spk2id: BTreeMap<String, u32>,
    pub id2spk: BTreeMap<String, String>,
    pub style2id: BTreeMap<String, u32>,
}
#[derive(Debug, Clone)]
pub struct Catalog(BTreeMap<u32, ModelInfo>);
pub use sungeki_project::Sbv2Selection as Selection;
fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
pub(super) fn names(map: &BTreeMap<String, u32>) -> bool {
    !map.is_empty()
        && map.len() <= 4096
        && map.keys().all(|s| text(s, 1024))
        && map.values().collect::<BTreeSet<_>>().len() == map.len()
}
impl Catalog {
    pub fn parse(value: serde_json::Value) -> Result<Self, Error> {
        let object = value.as_object().ok_or(Error::InvalidResponse)?;
        if object.len() > 4096 {
            return Err(Error::InvalidResponse);
        }
        let mut models = BTreeMap::new();
        for (key, value) in object {
            let id: u32 = key.parse().map_err(|_| Error::InvalidResponse)?;
            if id.to_string() != *key {
                return Err(Error::InvalidResponse);
            }
            let model: ModelInfo =
                serde_json::from_value(value.clone()).map_err(|_| Error::InvalidResponse)?;
            if !text(&model.config_path, 4096)
                || !text(&model.model_path, 4096)
                || !text(&model.device, 128)
                || !names(&model.spk2id)
                || !names(&model.style2id)
                || model.id2spk.len() != model.spk2id.len()
                || model
                    .spk2id
                    .iter()
                    .any(|(name, id)| model.id2spk.get(&id.to_string()) != Some(name))
            {
                return Err(Error::InvalidResponse);
            }
            models.insert(id, model);
        }
        Ok(Self(models))
    }
    pub fn models(&self) -> &BTreeMap<u32, ModelInfo> {
        &self.0
    }
    pub fn select(&self, settings: &Settings) -> Result<Selection, Error> {
        settings.validate().map_err(|_| Error::Configuration)?;
        let model = self.0.get(&settings.model_id).ok_or(Error::Configuration)?;
        let speaker_name = model
            .id2spk
            .get(&settings.speaker_id.to_string())
            .ok_or(Error::Configuration)?;
        let style_id = model
            .style2id
            .get(&settings.style)
            .ok_or(Error::Configuration)?;
        // Sorted maps make order-only changes irrelevant. Device changes remain material.
        let bytes = serde_json::to_vec(&("vvm-sbv2-server-metadata-v1", settings.model_id, model))
            .map_err(|_| Error::InvalidResponse)?;
        Ok(Selection {
            model_id: settings.model_id,
            speaker_id: settings.speaker_id,
            speaker_name: speaker_name.clone(),
            style: settings.style.clone(),
            style_id: *style_id,
            metadata_sha256: format!("{:x}", Sha256::digest(bytes)),
        })
    }
}
