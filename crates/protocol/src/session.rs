#[path = "runtime_settings.rs"]
mod runtime_settings;
pub use runtime_settings::*;
#[path = "presets.rs"]
mod presets;
pub use presets::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_editor::EditRequest;
use sungeki_project::Project;
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionRequest {
    GetLocalSpeechPolicy {
        args: crate::LocalSpeechPolicyArgs,
    },
    GetLocalSpeechCatalog {
        args: crate::LocalSpeechCatalogArgs,
    },
    ReadEditingAudio {
        session_id: String,
        expected_revision: u32,
        clip_id: String,
        offset: u32,
        length: u32,
    },
    ParseMarkdownScript {
        args: crate::ParseMarkdownScriptArgs,
    },
    InstantiatePreset {
        args: crate::InstantiatePresetArgs,
    },
    Presets {
        request: PresetRequest,
    },
    InstallSpeechConfig {
        args: crate::InstallSpeechConfigArgs,
    },
    GetRuntimeSettings,
    SaveRuntimeSettings {
        args: crate::SaveRuntimeSettingsArgs,
    },
    Proxy {
        request: crate::ProxyRequest,
    },
    ExportSubtitles {
        args: crate::ExportSubtitlesArgs,
    },
    BuildVideo {
        request: crate::BuildVideoRequest,
    },
    FramePreview {
        request: crate::FramePreviewRequest,
    },
    Collection {
        request: crate::CollectionRequest,
    },
    AudioPreview {
        request: crate::AudioPreviewRequest,
    },
    VideoPreview {
        args: crate::VideoPreviewArgs,
    },
    ProbeVrm {
        path: String,
        kind: sungeki_project::VrmKind,
    },
    ProbeVideo {
        path: String,
    },
    ProbeAudio {
        path: String,
    },
    ProposeSpeechGain {
        args: crate::ProposeSpeechGainArgs,
    },
    LoudnessJob {
        request: crate::LoudnessJobRequest,
    },
    AnalyzeAudio {
        args: crate::AnalyzeAudioArgs,
    },
    PsdPreview {
        session_id: String,
        expected_revision: u32,
        clip_id: String,
        time_us: sungeki_project::TimeUs,
    },
    ProbePsd {
        path: String,
    },
    RenderJob {
        request: crate::RenderJobRequest,
    },
    SpeechJob {
        request: crate::SpeechJobRequest,
    },
    GetVoicevoxSpeaker {
        args: crate::GetVoicevoxSpeakerArgs,
    },
    DiscoverVoicevox {
        args: crate::DiscoverVoicevoxArgs,
    },
    GetVoicevoxCatalog {
        args: crate::GetVoicevoxCatalogArgs,
    },
    PreviewPronunciation {
        session_id: String,
        expected_revision: u32,
        subtitle_text: String,
        spoken_text: Option<String>,
    },
    ReadReviewImage {
        args: crate::ReadReviewImageArgs,
    },
    StartReview {
        #[serde(default)]
        #[ts(optional)]
        sampling: Option<crate::ReviewFrameOptions>,
        session_id: String,
        job_id: String,
        expected_revision: u32,
        #[serde(default)]
        wait_ms: u32,
    },
    GetReviewJob {
        job_id: String,
        wait_ms: u32,
    },
    CancelReviewJob {
        job_id: String,
    },
    ListReviewJobs {
        session_id: Option<String>,
        after: Option<String>,
        limit: u32,
    },
    ReadVrmAsset {
        session_id: String,
        expected_revision: u32,
        asset_id: String,
        offset: u32,
        length: u32,
    },
    ImagePreview {
        session_id: String,
        asset_id: String,
        expected_revision: u32,
    },
    ProbeImage {
        path: String,
    },
    ReadProject {
        session_id: String,
        expected_revision: Option<u32>,
        query: crate::ProjectQuery,
    },
    PreviewEdit {
        session_id: String,
        expected_revision: u32,
        commands: Vec<sungeki_editor::Command>,
        offset: u32,
        limit: u32,
    },
    Checkpoint {
        session_id: String,
        request: crate::CheckpointRequest,
    },
    Create {
        title: String,
    },
    Open {
        path: String,
        #[serde(default)]
        import_checkpoints: bool,
    },
    Save {
        session_id: String,
        path: Option<String>,
        expected_revision: u32,
    },
    List {},
    Get {
        session_id: String,
    },
    Edit {
        session_id: String,
        edit: EditRequest,
    },
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SessionSnapshot {
    pub event_cursor: crate::EventCursor,
    pub session_id: String,
    pub project: Project,
    pub revision: u32,
    pub document_path: Option<String>,
    pub dirty: bool,
    pub undo_operation_id: Option<String>,
    pub redo_operation_id: Option<String>,
}

#[path = "session_response.rs"]
mod response;
pub use response::SessionResponse;
