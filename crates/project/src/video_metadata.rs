//! Source metadata only. Decoder, display transform and PTS support are separate gates.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Exact ratio; rates alone do not establish constant frame timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MediaRatio {
    pub numerator: u32,
    pub denominator: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MediaStreamClock {
    pub time_base: MediaRatio,
    /// Original timestamps, including a negative origin. Not rebased to zero.
    #[ts(type = "number | null")]
    pub start_pts: Option<i64>,
    #[ts(type = "number | null")]
    pub duration_ts: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VideoStreamInfo {
    pub index: u32,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub sample_aspect_ratio: Option<MediaRatio>,
    pub clock: MediaStreamClock,
    pub average_frame_rate: Option<MediaRatio>,
    pub nominal_frame_rate: Option<MediaRatio>,
    #[ts(type = "number | null")]
    pub frame_count: Option<u64>,
    /// Preserve the matrix too: rotation alone cannot describe a reflection.
    pub rotation_degrees: Option<f64>,
    pub display_matrix: Option<String>,
    pub legacy_rotate_tag: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VideoAudioStreamInfo {
    pub index: u32,
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub clock: MediaStreamClock,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum VideoContainer {
    /// QuickTime/MOV and MP4-family containers.
    Mov,
    /// Matroska and its WebM subset.
    Matroska,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VideoMetadata {
    pub container: VideoContainer,
    pub video: VideoStreamInfo,
    pub audio: Vec<VideoAudioStreamInfo>,
}
