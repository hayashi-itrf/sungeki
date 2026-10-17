use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ImageProbe {
    pub format: ImageFormat,
    /// Encoded dimensions, before applying EXIF orientation.
    pub width: u32,
    pub height: u32,
    pub orientation: u8,
    pub has_alpha: bool,
    #[ts(type = "number")]
    pub byte_length: u64,
    pub sha256: String,
}
