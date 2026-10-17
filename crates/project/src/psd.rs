use crate::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PsdLayerKind {
    Pixel,
    Group,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PsdLayer {
    pub path: String,
    pub name: String,
    pub kind: PsdLayerKind,
    pub visible: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PsdProbe {
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    #[ts(type = "number")]
    pub byte_length: u64,
    pub layer_layout_version: u32,
    pub layers: Vec<PsdLayer>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PsdSelection {
    pub group: String,
    pub option: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PsdPose {
    pub source_sha256: String,
    pub selections: Vec<PsdSelection>,
}

fn invalid(message: &str) -> ValidationError {
    ValidationError::new(message.into())
}
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map(|v| v.0).unwrap_or("")
}
fn valid_path(path: &str) -> bool {
    let Some(path) = path.strip_prefix('/') else {
        return false;
    };
    let parts: Vec<_> = path.split('/').collect();
    parts.len() <= 33
        && parts.iter().all(|part| {
            part.parse::<u32>()
                .ok()
                .is_some_and(|n| n < 4096 && n.to_string() == *part)
        })
}
impl PsdProbe {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !(1..=8192).contains(&self.width)
            || !(1..=8192).contains(&self.height)
            || u64::from(self.width) * u64::from(self.height) > 32 * 1024 * 1024
            || !(26..=64 * 1024 * 1024).contains(&self.byte_length)
            || self.layer_layout_version != 1
            || self.layers.is_empty()
            || self.layers.len() > 4096
        {
            return Err(invalid("PSD原本の検査情報が不正または未対応です"));
        }
        let mut layers = HashMap::new();
        for layer in &self.layers {
            if !valid_path(&layer.path)
                || layer.name.len() > 4096
                || layer.name.contains('\0')
                || layers.insert(layer.path.as_str(), layer).is_some()
            {
                return Err(invalid("PSDレイヤーのパス・名前が不正または重複しています"));
            }
        }
        for layer in &self.layers {
            let parent = parent(&layer.path);
            if !parent.is_empty()
                && layers
                    .get(parent)
                    .is_none_or(|p| p.kind != PsdLayerKind::Group)
            {
                return Err(invalid("PSDレイヤーの親グループがありません"));
            }
        }
        Ok(())
    }
}
impl PsdPose {
    pub fn validate(&self, probe: &PsdProbe) -> Result<(), ValidationError> {
        probe.validate()?;
        if self.source_sha256 != probe.sha256 || self.selections.len() > 256 {
            return Err(invalid("PSDポーズの原本hashが違うか選択が多すぎます"));
        }
        let layers: HashMap<_, _> = probe.layers.iter().map(|l| (l.path.as_str(), l)).collect();
        let mut children: HashMap<&str, Vec<&PsdLayer>> = HashMap::new();
        for layer in &probe.layers {
            children.entry(parent(&layer.path)).or_default().push(layer);
        }
        let mut visible: HashMap<_, _> = probe
            .layers
            .iter()
            .map(|l| (l.path.as_str(), l.visible))
            .collect();
        let mut selections: Vec<_> = self.selections.iter().collect();
        selections.sort_by_key(|s| s.group.matches('/').count());
        let mut seen = HashSet::new();
        for selection in selections {
            let Some(group) = layers.get(selection.group.as_str()) else {
                return Err(invalid("PSD選択グループがありません"));
            };
            let Some(option) = layers.get(selection.option.as_str()) else {
                return Err(invalid("PSD選択肢がありません"));
            };
            if group.kind != PsdLayerKind::Group
                || parent(&option.path) != group.path
                || !option.name.starts_with('*')
                || !seen.insert(&group.path)
            {
                return Err(invalid("PSDポーズの選択先が不正または重複しています"));
            }
            let mut ancestor = group.path.as_str();
            while !ancestor.is_empty() {
                if !visible[ancestor] {
                    return Err(invalid("PSDポーズの選択グループが非表示です"));
                }
                ancestor = parent(ancestor);
            }
            for layer in children.get(group.path.as_str()).into_iter().flatten() {
                if layer.name.starts_with('*') {
                    visible.insert(&layer.path, layer.path == option.path);
                }
            }
        }
        Ok(())
    }
}
