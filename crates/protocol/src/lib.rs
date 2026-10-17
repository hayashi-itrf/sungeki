mod error;
pub mod framing;
mod handshake;
mod message;

pub use error::{ApiError, ErrorCode};
pub use handshake::{Capabilities, ClientHello, HostInfo};
pub use message::{Request, RequestBody, Response, ResponseBody};

pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 148;
pub const APPLICATION: &str = "sungeki-host";
pub const BUILD_ID: &str = concat!(env!("CARGO_PKG_VERSION"), "-vrm-editor-148");
pub const SOCKET_NAME: &str = "host.sock";

mod session;
pub use session::*;
mod markdown_script;
pub use markdown_script::*;
mod mcp_args;
pub use mcp_args::*;
pub use sungeki_editor::{EditAction, EditRequest};

mod events;
pub use events::*;

mod checkpoints;
pub use checkpoints::*;

mod blur_minor;
mod compressed_audio_minor;
mod minimum_minor;
mod motion_minor;
mod psd_minor;
mod scene_fade_minor;

mod edit_preview;
pub use edit_preview::EditPreview;

mod project_read;
pub use project_read::*;

mod review_frames;
pub use review_frames::*;
mod review_job;
pub use review_job::*;

mod job_args;
pub use job_args::*;
mod pronunciation;
pub use pronunciation::*;

mod voice_catalog;
pub use voice_catalog::*;

mod sbv2_profile;
pub use sbv2_profile::*;
mod speech_job;
pub use speech_job::*;
mod render_job;
pub use render_job::*;

#[cfg(test)]
mod minimum_minor_tests;

mod lipsync_minor;
mod sbv2_audio_minor;

mod audio_analysis;
pub use audio_analysis::*;

mod loudness_job;
pub use loudness_job::*;

mod gain_proposal;
pub use gain_proposal::*;

mod preview;
pub use preview::*;

mod speech_minor;

mod volume_minor;

mod build_video;
pub use build_video::*;

mod subtitles;
pub use subtitles::*;

#[cfg(test)]
mod portrait_minor_tests;

#[cfg(test)]
mod sbv2_minor_tests;

mod collection;
pub use collection::*;
mod collection_plan;
pub use collection_plan::*;

mod minimum_minor_vrm;

#[cfg(test)]
mod ripple_minor_tests;

mod lifecycle;
pub use lifecycle::*;
