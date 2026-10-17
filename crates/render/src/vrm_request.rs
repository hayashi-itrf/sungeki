use crate::{RenderError, plan::error};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use sungeki_project::{TimeUs, VrmAssetProbe, VrmKind};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VrmSource {
    pub path: PathBuf,
    pub sha256: String,
    pub byte_length: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VrmFrameRequest {
    pub time_us: TimeUs,
    pub expressions: BTreeMap<String, f64>,
    #[serde(default)]
    pub pose: BTreeMap<String, [f64; 3]>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VrmRenderRequest {
    pub version: u32,
    pub model: VrmSource,
    pub motion: VrmSource,
    pub width: u32,
    pub height: u32,
    pub frames: Vec<VrmFrameRequest>,
}
impl VrmRenderRequest {
    pub fn validate(&self) -> Result<(), RenderError> {
        if self.version != 1
            || !(1..=2048).contains(&self.width)
            || !(1..=2048).contains(&self.height)
            || self.frames.is_empty()
            || self.frames.len() > 100000
        {
            return Err(error("VRM描画要求の版・寸法・件数が不正です"));
        }
        for (source, kind) in [
            (&self.model, VrmKind::Model),
            (&self.motion, VrmKind::Motion),
        ] {
            if !source.path.is_absolute() {
                return Err(error("VRM原本は絶対パスが必要です"));
            }
            VrmAssetProbe {
                sha256: source.sha256.clone(),
                byte_length: source.byte_length,
                inspection: sungeki_project::VrmInspection {
                    kind,
                    version: "1.0".into(),
                    expressions: vec![],
                },
            }
            .validate()
            .map_err(error)?;
        }
        for f in &self.frames {
            sungeki_project::validate_pose(&f.pose).map_err(error)?;
            if f.time_us.0 > 7_200_000_000
                || f.expressions.len() > 256
                || f.expressions.iter().any(|(k, v)| {
                    k.is_empty()
                        || k.len() > 256
                        || k.chars().any(char::is_control)
                        || !v.is_finite()
                        || !(0.0..=1.0).contains(v)
                })
            {
                return Err(error("VRM描画時刻・表情が不正です"));
            }
        }
        Ok(())
    }
}
