mod build_fitting;
mod media;
pub use build_fitting::*;
pub use media::*;
mod pronunciation;
pub use pronunciation::*;
mod model;
mod validate;
pub use model::*;
pub use validate::validate;
mod validation_error;
pub use validation_error::{ValidationDiagnostic, ValidationError};
mod checkpoint;
pub use checkpoint::{Checkpoint, CheckpointAsset, CheckpointSummary};
mod image_probe;
pub use image_probe::{ImageFormat, ImageProbe};
mod asset_validation;
mod review;
pub use review::*;
mod job;
pub use job::*;
mod qwen;
pub use qwen::{QwenAudioSource, QwenSelection, QwenSettings};
mod irodori;
pub use irodori::{IrodoriAudioSource, IrodoriIdentity, IrodoriSettings};
mod qwen_identity;
pub use qwen_identity::QwenIdentity;
mod sbv2_identity;
pub use sbv2_identity::Sbv2Identity;
mod sbv2_audio;
pub use sbv2_audio::{Sbv2AudioSource, Sbv2Selection};
mod sbv2;
pub use sbv2::{Sbv2Language, Sbv2Settings};
mod voicevox;
pub use voicevox::VoicevoxSettings;
mod character;
pub use character::{
    Character, SubtitleLayout, SubtitleOutline, SubtitleStyle, TextAlignment, Voice,
};
mod audio_probe;
pub use audio_probe::{AudioProbe, CompressedAudioEncoding};
mod speech_audio;
pub use speech_audio::SpeechAudio;
mod audio_relocation;
pub use audio_relocation::AudioRelocation;
mod psd;
pub use psd::{PsdLayer, PsdLayerKind, PsdPose, PsdProbe, PsdSelection};
mod lipsync;
pub use lipsync::{
    LIPSYNC_ANALYSIS_VERSION, LipSyncAnalysis, LipSyncSettings, MAX_LIPSYNC_WINDOWS, MouthInterval,
};
mod psd_speech;
pub use psd_speech::PsdSpeechBinding;
// Shared bounds for stored speech gain and source-relative gain proposals.
pub const MIN_SPEECH_GAIN_DB: f64 = -60.0;
pub const MAX_SPEECH_GAIN_DB: f64 = 24.0;
mod audio_playback;
pub use audio_playback::AudioPlayback;
mod ducking;
pub use ducking::Ducking;

mod visual_fade;
pub use visual_fade::VisualFade;

mod visual_motion;
pub use visual_motion::{MotionEasing, VisualMotion};

mod volume_point;
pub use volume_point::VolumePoint;

mod build_psd_states;
mod build_recipe;
mod build_recipe_validation;
mod build_standing_validation;
mod preset;
pub use build_recipe::*;
pub use preset::{Preset, PresetContent};

mod video_metadata;
pub use video_metadata::{
    MediaRatio, MediaStreamClock, VideoAudioStreamInfo, VideoContainer, VideoMetadata,
    VideoStreamInfo,
};
mod video_probe;
pub use video_probe::VideoProbe;

mod video_playback;
pub use video_playback::VideoPlayback;

mod video_display;
pub use video_display::VideoDisplayGeometry;

mod sprite;
pub use sprite::{SpritePose, SpriteSet};

mod vrm;
pub use vrm::{VrmAssetProbe, VrmInspection, VrmKind};

mod vrm_standing;
pub use vrm_standing::{VrmStanding, validate_pose};

mod local_voice;
pub use local_voice::{LocalSpeechEngine, LocalVoiceSettings};
