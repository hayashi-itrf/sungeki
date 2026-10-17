//! Stored source metadata; no filesystem or GPU calls.
use crate::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum VrmKind {
    Model,
    Motion,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VrmInspection {
    pub kind: VrmKind,
    pub version: String,
    pub expressions: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VrmAssetProbe {
    pub sha256: String,
    #[ts(type = "number")]
    pub byte_length: u64,
    pub inspection: VrmInspection,
}
impl VrmAssetProbe {
    pub fn validate(&self) -> Result<(), ValidationError> {
        let i = &self.inspection;
        let version_ok = match i.kind {
            VrmKind::Model => matches!(i.version.as_str(), "0.0" | "1.0"),
            VrmKind::Motion => i.version == "1.0" && i.expressions.is_empty(),
        };
        let mut names = std::collections::HashSet::new();
        if !version_ok
            || !(20..=100 * 1024 * 1024).contains(&self.byte_length)
            || self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || i.expressions.len() > 256
            || i.expressions.iter().any(|s| {
                s.is_empty() || s.len() > 256 || s.chars().any(char::is_control) || !names.insert(s)
            })
        {
            return Err(ValidationError::new("VRM/VRMA原本情報が不正です".into()));
        }
        Ok(())
    }
}
