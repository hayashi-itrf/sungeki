use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::*;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Move explicitly selected clips starting at/after the boundary, preserving durations.
    Ripple {
        clip_ids: Vec<String>,
        boundary_us: TimeUs,
        #[ts(type = "number")]
        delta_us: i64,
    },
    /// Fit the existing layout inside a new canvas, preserving aspect ratio.
    ResizeLayout {
        width: u32,
        height: u32,
    },
    ApplyVoicePreset {
        preset_id: String,
        preset_revision: u32,
        preset: Preset,
        character_id: String,
        credit: String,
    },
    ApplyCharacterPreset {
        preset_id: String,
        preset_revision: u32,
        preset: Preset,
        ids: crate::CharacterPresetIds,
        placement: Option<crate::PresetStandingPlacement>,
    },
    SetReactionMotion {
        clip_id: String,
        center_x: f64,
        center_y: f64,
        width: f64,
        height: f64,
        start_scale: f64,
        fade: VisualFade,
    },
    PrependForeword {
        template: crate::ForewordTemplate,
    },
    RefreshSceneCredits {
        clip_id: String,
        scene_id: String,
    },
    FitVisualToSpeech {
        clip_id: String,
        first_speech_id: String,
        last_speech_id: String,
    },
    ApplyDialogueTemplate {
        template: crate::DialogueTemplate,
    },
    LayoutSpeech {
        clip_ids: Vec<String>,
        start_us: TimeUs,
        gap_us: TimeUs,
    },
    AppendSpeechScript {
        script: crate::SpeechScript,
    },
    SetAudioMix {
        audio_mix: AudioMix,
    },
    SetAudio {
        clip_id: String,
        asset_id: String,
        playback: AudioPlayback,
    },
    SetSpeechGain {
        clip_id: String,
        gain_db: Option<f64>,
    },
    SetPsdSpeech {
        clip_id: String,
        speech: Option<PsdSpeechBinding>,
    },
    SetPsd {
        clip_id: String,
        asset_id: String,
        pose: PsdPose,
    },
    SetSpeechAudio {
        clip_id: String,
        audio: Option<SpeechAudio>,
    },
    SetCharacter {
        character: Character,
    },
    RemoveCharacter {
        character_id: String,
    },
    SetSpeech {
        clip_id: String,
        character_id: String,
        subtitle_text: String,
        spoken_text: Option<String>,
    },
    SetPronunciationDictionary {
        entries: Vec<PronunciationEntry>,
    },
    RegisterAsset {
        asset: Asset,
    },
    SetAssetMetadata {
        asset_id: String,
        name: String,
        source: Option<String>,
        credit: String,
    },
    RelinkAsset {
        asset_id: String,
        locator: AssetLocator,
        probe: AssetProbe,
    },
    RemoveAsset {
        asset_id: String,
    },
    SetVrm {
        clip_id: String,
        standing: sungeki_project::VrmStanding,
    },
    SetSprite {
        clip_id: String,
        sprites: sungeki_project::SpriteSet,
        pose: Option<String>,
        character_id: Option<String>,
    },
    SetVideo {
        clip_id: String,
        asset_id: String,
        playback: VideoPlayback,
    },
    SetImageAsset {
        clip_id: String,
        asset_id: String,
    },
    SetSceneFade {
        scene_id: String,
        fade: Option<VisualFade>,
    },
    RenameScene {
        scene_id: String,
        name: String,
    },
    ResizeScene {
        scene_id: String,
        duration_us: TimeUs,
    },
    DeleteScene {
        scene_id: String,
    },
    UngroupScene {
        scene_id: String,
    },
    LayoutScenes {
        scene_ids: Vec<String>,
        start_us: TimeUs,
        gap_us: TimeUs,
    },
    RenameGroup {
        group_id: String,
        name: String,
    },
    MoveGroup {
        group_id: String,
        start_us: TimeUs,
    },
    MoveSelection {
        scene_ids: Vec<String>,
        group_ids: Vec<String>,
        clip_ids: Vec<String>,
        #[ts(type = "number")]
        delta_us: i64,
    },
    DuplicateScene {
        scene_id: String,
        new_scene_id: String,
        start_us: TimeUs,
        clip_ids: std::collections::BTreeMap<String, String>,
        group_ids: std::collections::BTreeMap<String, String>,
    },
    DuplicateGroup {
        group_id: String,
        new_group_id: String,
        start_us: TimeUs,
        clip_ids: std::collections::BTreeMap<String, String>,
    },
    ReplaceProject {
        checkpoint_id: Option<String>,
        project: Project,
    },
    SetCanvas {
        canvas: Canvas,
    },
    RenameProject {
        title: String,
    },
    AddTrack {
        track: Track,
    },
    SetTrackProperties {
        track_id: String,
        name: String,
        visible: bool,
        muted: bool,
    },
    ReorderTracks {
        track_ids: Vec<String>,
    },
    SetClipTrack {
        clip_id: String,
        track_id: String,
    },
    AddClip {
        clip: Clip,
    },
    DuplicateClip {
        clip_id: String,
        new_clip_id: String,
        start_us: TimeUs,
    },
    SetText {
        clip_id: String,
        text: String,
    },
    SetTextOutline {
        clip_id: String,
        outline: Option<SubtitleOutline>,
    },
    SetTextLayout {
        clip_id: String,
        layout: Option<SubtitleLayout>,
    },
    SetTextStyle {
        clip_id: String,
        font_size: f64,
        color: String,
    },
    SetBlur {
        clip_id: String,
        sigma: f64,
    },
    SetShape {
        clip_id: String,
        shape: ShapeKind,
        fill: String,
    },
    SetTransform {
        clip_id: String,
        transform: Transform,
    },
    MoveClip {
        clip_id: String,
        start_us: TimeUs,
    },
    ResizeClip {
        clip_id: String,
        duration_us: TimeUs,
    },
    SetClipScene {
        clip_id: String,
        scene_id: Option<String>,
    },
    DeleteClip {
        clip_id: String,
    },
    AddScene {
        scene: Scene,
    },
    MoveScene {
        scene_id: String,
        start_us: TimeUs,
    },
    AddGroup {
        group: Group,
    },
    Ungroup {
        group_id: String,
    },
}
