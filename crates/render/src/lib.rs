//! Blocking render worker. The host owns jobs, freshness checks, cancellation,
//! exclusive staging directories and final publication. No Project mutation.
mod chapters;
mod plan;
pub use plan::{PreviewPlan, RenderError, RenderPlan};
#[cfg(unix)]
mod execute;
#[cfg(unix)]
mod frame;
#[cfg(unix)]
mod prepare;
#[cfg(unix)]
pub use frame::{CURRENT_LAYOUT_VERSION, FrameArtifact, FramePlan, render_frame_staged};
#[cfg(unix)]
mod originals;
#[cfg(unix)]
mod verify;
#[cfg(unix)]
pub use {
    execute::{RenderArtifact, RenderProgress, RenderRuntime, render_staged},
    plan::PreviewArtifact,
};
#[cfg(unix)]
mod frame_cache;
#[cfg(all(test, unix))]
mod original_tests;
#[cfg(unix)]
pub use frame_cache::FrameCacheStats;
mod audio_graph;
#[cfg(all(test, unix))]
mod audio_graph_tests;
#[cfg(unix)]
mod audio_mix;
#[cfg(all(test, unix))]
mod audio_resample_tests;
#[cfg(unix)]
mod decoded_audio;
mod ducking;
#[cfg(all(test, unix))]
mod ducking_limit_tests;
#[cfg(all(test, unix))]
mod ducking_tests;
#[cfg(unix)]
mod master_audio;
#[cfg(unix)]
mod master_policy;
#[cfg(all(test, unix))]
mod master_tests;
mod volume_envelope;

#[cfg(unix)]
mod audio_preview;
#[cfg(unix)]
pub use audio_preview::{AUDIO_PREVIEW_CHUNK, audio_preview_plan, audio_preview_staged};

mod subtitles;
#[cfg(unix)]
mod video_segments;
#[cfg(all(test, unix))]
mod video_segments_tests;
pub use subtitles::{SubtitleFormat, subtitle_document};
mod text_report;
pub use text_report::TextLayoutIssue;

#[cfg(unix)]
mod text_inspection;
#[cfg(unix)]
pub use text_inspection::{TextInspection, TextObservation, inspect_text_layout};

#[cfg(unix)]
mod text_margin;

#[cfg(unix)]
mod audio_inspection;
#[cfg(unix)]
pub use audio_inspection::inspect_output_audio;

#[cfg(unix)]
mod video_frame;
#[cfg(unix)]
pub use video_frame::{VideoFrameResource, VideoRaster, stage_video_clip_frame, stage_video_frame};

#[cfg(unix)]
mod video_resolver;
#[cfg(unix)]
pub use video_resolver::{ResolvedVideoLayers, VideoLayerResolver};

#[cfg(unix)]
mod prepared_visuals;

mod audio_selection;
mod video_audio_selection;
pub use video_audio_selection::VideoAudioSelection;

#[cfg(unix)]
mod vrm_output;
#[cfg(unix)]
pub mod vrm_request;
#[cfg(unix)]
pub mod vrm_worker;

#[cfg(unix)]
mod execution_runtime;
#[cfg(unix)]
pub use execution_runtime::AudioRuntime;
