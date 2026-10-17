use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::{Canvas, Clip, Scene, TimeUs, Track};
use ts_rs::TS;
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProjectView {
    Full,
    Summary,
    Scenes,
    Timeline,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProjectQuery {
    pub view: ProjectView,
    pub scene_ids: Option<Vec<String>>,
    pub clip_ids: Option<Vec<String>>,
    pub offset: u32,
    pub limit: u32,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProjectCounts {
    pub tracks: usize,
    pub scenes: usize,
    pub groups: usize,
    pub clips: usize,
    pub assets: usize,
    pub characters: usize,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct SceneEntry {
    pub scene: Scene,
    pub clip_count: usize,
    pub group_count: usize,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct TimelineEntry {
    pub clip: Clip,
    pub group_id: Option<String>,
    pub track_order: usize,
    pub clip_order: usize,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "view", rename_all = "snake_case")]
pub enum ProjectViewData {
    Summary {
        title: String,
        canvas: Canvas,
        duration_us: TimeUs,
        counts: ProjectCounts,
        dirty: bool,
        document_path: Option<String>,
    },
    Scenes {
        entries: Vec<SceneEntry>,
        total: usize,
        next_offset: Option<u32>,
    },
    Timeline {
        entries: Vec<TimelineEntry>,
        tracks: Vec<Track>,
        total: usize,
        next_offset: Option<u32>,
    },
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProjectReadResult {
    pub session_id: String,
    pub project_id: String,
    pub revision: u32,
    pub event_cursor: crate::EventCursor,
    pub data: ProjectViewData,
}
