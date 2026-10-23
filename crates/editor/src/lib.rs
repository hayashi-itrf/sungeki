mod commands;
mod resize_layout;
pub use resize_layout::plan_resize_layout;
mod preset;
mod voice_preset;
pub use voice_preset::plan_voice_preset;
mod preset_geometry;
pub use preset::{CharacterPresetIds, PresetStandingPlacement, plan_character_preset};
mod history;
mod transaction;
pub use commands::Command;
pub use history::{History, HistoryError};
pub use transaction::apply_batch;
mod operation;
pub use operation::{EditAction, EditReceipt, EditRequest, EditSession, OperationError};

mod comparison;
pub use comparison::{DifferenceKind, ProjectDifference, ValuePreview, compare_projects};

mod duplicate;
mod scene_edit;
mod speech_edit;
mod speech_timing;
pub use speech_timing::{SpeechTimingError, SpeechTimingPlan, plan_speech_timing};
mod track_collision;
mod track_edit;
mod track_separation;

mod audio_relocation;
pub use audio_relocation::AudioRelocations;

mod speech_script;
pub use speech_script::{SpeechScript, SpeechScriptLine};

mod build_script;
pub use build_script::{BuildScriptIds, plan_build_script};
mod markdown_script;
pub use markdown_script::{MarkdownScene, MarkdownScriptError, parse_markdown_script};

mod speech_layout;

mod dialogue_geometry;
mod dialogue_template;
mod standing_facing;
pub use dialogue_template::{DialogueParticipant, DialogueTemplate};

mod visual_timing;

mod credits;

mod visual_content;

mod foreword;
pub use foreword::ForewordTemplate;

mod build_speech;
pub use build_speech::plan_build_speech_application;

mod build_layout;
mod build_visuals;
pub use build_layout::{
    BuildForewordIds, BuildLayoutIds, BuildOpeningIds, BuildSceneVisualIds, plan_build_layout,
};

mod build_portrait;

mod video_edit;

mod reaction;

mod asset_relink;
mod recovery_relink;

mod batch_references;
pub use batch_references::{ReferenceCreationPlan, plan_batch_references};

mod reference_allocation;
pub use reference_allocation::has_batch_references;
mod reference_commands;

mod edit_outcome;
pub use edit_outcome::EditSummary;

mod ripple;
