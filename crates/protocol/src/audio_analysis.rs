use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AnalyzeAudioArgs {
    pub session_id: String,
    pub expected_revision: u32,
    pub asset_id: String,
    pub base_frames: u32,
    pub level: u32,
    pub channel: u16,
    pub start_bucket: u32,
    pub count: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AudioAnalysisSignal {
    SourcePreGain,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AudioWaveformLevel {
    #[ts(type = "number")]
    pub frames_per_bucket: u64,
    pub bucket_count: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AudioAnalysis {
    pub session_id: String,
    pub source_revision: u32,
    pub asset_id: String,
    pub source_sha256: String,
    pub version: u32,
    pub signal: AudioAnalysisSignal,
    pub sample_rate: u32,
    pub channels: u16,
    #[ts(type = "number")]
    pub source_frames: u64,
    pub rms: Vec<f64>,
    pub sample_peak: Vec<f64>,
    pub lufs_measured: bool,
    pub true_peak_measured: bool,
    pub cache_hit: bool,
    pub levels: Vec<AudioWaveformLevel>,
    pub level: u32,
    pub channel: u16,
    pub start_bucket: u32,
    #[ts(type = "number")]
    pub start_frame: u64,
    #[ts(type = "number")]
    pub end_frame: u64,
    pub peaks: Vec<[f64; 2]>,
}
