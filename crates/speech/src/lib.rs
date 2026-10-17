//! Shared speech input preparation and engine adapters. No Project mutations.
mod text;
pub use text::{AppliedPronunciation, ResolvedText, TextError, TextSource, resolve_text};
pub use text::{MAX_SPEECH_INPUT_BYTES, MAX_SPEECH_OUTPUT_BYTES};
mod input;
pub mod voicevox;
pub use input::{InputError, PreparedVoicevoxInput, VoicevoxIdentity};
mod clip_input;
mod target;
pub use clip_input::PreparedSpeechClip;

mod job_input;
pub use job_input::SpeechJobInput;

mod audio_reference;
pub use audio_reference::audio_reference_matches;

pub mod sbv2;

mod response;
pub use response::SpeechResponse;

pub mod irodori;
pub mod qwen;
