//! Immutable, I/O-free scene evaluation shared by output and formal preview.
mod ducking;
mod plan;
mod timing;
pub use ducking::DuckingEnvelope;
pub use plan::{Layer, Primitive, ScenePlan, VisualSpan};
mod audio_cue;
pub use audio_cue::{AudioCue, AudioSource};
pub use timing::{FrameSpan, first_frame_at_or_after};

mod mouth_timing;
mod psd_speech;
mod sprite;
pub use sprite::sprite_layers;

mod visual_fade;

mod scene_fade;

mod video_timing;
pub use video_timing::VideoClipTiming;

mod video_layers;

mod vrm_layers;
