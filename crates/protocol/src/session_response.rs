use super::{PresetRecord, PresetSummary, SessionSnapshot};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_editor::EditReceipt;
use ts_rs::TS;
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionResponse {
    LocalSpeechPolicy {
        policy: String,
    },
    LocalSpeechCatalog {
        catalog: crate::LocalSpeechCatalog,
    },
    EditingAudioBytes {
        offset: u32,
        total: u32,
        base64: String,
    },
    MarkdownScript {
        scenes: Vec<sungeki_editor::MarkdownScene>,
    },
    RenderFrame {
        image: crate::RenderFrame,
    },
    RenderBytes {
        data: crate::RenderBytes,
    },
    SpeechConfigInstalled {
        configuration_path: std::path::PathBuf,
    },
    RuntimeSettings {
        settings: crate::RuntimeSettings,
    },
    Preset {
        record: PresetRecord,
    },
    Presets {
        items: Vec<PresetSummary>,
        next_after_id: Option<String>,
    },
    ProxyRemoved {
        job_id: String,
        asset_id: String,
        removed: bool,
    },
    ProxyJob {
        job: crate::ProxyJob,
    },
    ProxyJobs {
        job_ids: Vec<String>,
    },
    SubtitlesExported {
        publication: crate::SubtitlePublication,
    },
    BuildVideoJob {
        job: crate::BuildVideoJob,
    },
    BuildReuseSource {
        source: crate::BuildReuseSource,
    },
    BuildVideoJobs {
        job_ids: Vec<String>,
    },
    FramePreviewBytes {
        data: crate::FramePreviewBytes,
    },
    FramePreviewPublished {
        publication: crate::FramePreviewPublication,
    },
    FramePreviewJob {
        job: crate::FramePreviewJob,
    },
    FramePreviewJobs {
        job_ids: Vec<String>,
    },
    CollectionPlanPage {
        page: crate::CollectionPlanPage,
    },
    CollectionJob {
        job: crate::CollectionJob,
    },
    CollectionJobs {
        job_ids: Vec<String>,
    },
    AudioPreviewJob {
        job: crate::AudioPreviewJob,
    },
    AudioPreviewJobs {
        job_ids: Vec<String>,
    },
    AudioPreviewBytes {
        data: crate::AudioPreviewBytes,
    },
    VrmProbed {
        probe: sungeki_project::VrmAssetProbe,
    },
    VideoProbed {
        probe: sungeki_project::VideoProbe,
    },
    AudioProbed {
        probe: sungeki_project::AudioProbe,
    },
    SpeechGainProposed {
        proposal: crate::SpeechGainProposal,
    },
    LoudnessJob {
        job: crate::LoudnessJob,
    },
    LoudnessJobs {
        job_ids: Vec<String>,
    },
    AudioAnalyzed {
        analysis: crate::AudioAnalysis,
    },
    RenderPublished {
        publication: crate::RenderPublication,
    },
    PsdProbed {
        probe: sungeki_project::PsdProbe,
    },
    RenderJob {
        job: crate::RenderJob,
    },
    RenderJobs {
        job_ids: Vec<String>,
    },
    SpeechJob {
        job: crate::SpeechJob,
    },
    SpeechJobs {
        job_ids: Vec<String>,
    },
    QwenProfile {
        details: crate::QwenProfileDetails,
    },
    QwenProfiles {
        profiles: Vec<crate::QwenProfileSummary>,
    },
    IrodoriProfile {
        details: crate::IrodoriProfileDetails,
    },
    IrodoriProfiles {
        profiles: Vec<crate::IrodoriProfileSummary>,
    },
    Sbv2Profile {
        details: crate::Sbv2ProfileDetails,
    },
    Sbv2Profiles {
        profiles: Vec<crate::Sbv2ProfileSummary>,
    },
    VoicevoxSpeaker {
        details: crate::VoicevoxSpeakerDetails,
    },
    VoicevoxEngine {
        engine: crate::VoicevoxEngineInfo,
    },
    VoicevoxCatalog {
        engine: crate::VoicevoxEngineInfo,
        core_version: String,
        speakers: Vec<crate::VoicevoxSpeakerInfo>,
    },
    PronunciationPreview {
        session_id: String,
        revision: u32,
        final_text: String,
        explicit_reading: bool,
        applied: Vec<crate::PronunciationUse>,
    },
    ReviewImageBytes {
        data: crate::FramePreviewBytes,
    },
    ReviewJob {
        job: crate::ReviewJob,
    },
    ReviewJobs {
        job_ids: Vec<String>,
    },
    VrmAssetBytes {
        offset: u32,
        total: u32,
        sha256: String,
        base64: String,
    },
    ImagePreview {
        asset_id: String,
        sha256: String,
        png_base64: String,
        width: u32,
        height: u32,
    },
    ImageProbed {
        probe: sungeki_project::ImageProbe,
    },
    ProjectRead {
        result: crate::ProjectReadResult,
    },
    EditPreview {
        preview: crate::EditPreview,
    },
    CheckpointDeleted {
        checkpoint_id: String,
    },
    CheckpointCompared {
        comparison: crate::CheckpointComparison,
    },
    CheckpointCreated {
        checkpoint: sungeki_project::CheckpointSummary,
    },
    Checkpoints {
        checkpoints: Vec<sungeki_project::CheckpointSummary>,
        next_after: Option<String>,
    },
    Snapshot {
        snapshot: SessionSnapshot,
    },
    Sessions {
        session_ids: Vec<String>,
    },
    Edited {
        receipt: EditReceipt,
        snapshot: SessionSnapshot,
    },
}
