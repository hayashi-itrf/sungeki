use serde::Serialize;
use sungeki_project::{AudioPlayback, SpeechAudio, TimeUs, VideoPlayback};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioCue {
    pub clip_id: String,
    pub start_us: TimeUs,
    pub end_us: TimeUs,
    pub source: AudioSource,
    pub gain_db: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AudioSource {
    Video {
        asset_id: String,
        playback: VideoPlayback,
    },
    Speech {
        audio: Option<SpeechAudio>,
    },
    Material {
        asset_id: String,
        playback: AudioPlayback,
    },
}
impl AudioSource {
    /// Decode identity is distinct from the original asset when it contains
    /// multiple selectable audio streams.
    pub fn resource_id(&self) -> Option<String> {
        match self {
            Self::Video { asset_id, playback } => playback
                .audio_stream
                .map(|stream| format!("video-audio:{asset_id}:{stream}")),
            _ => self.asset_id().map(str::to_owned),
        }
    }

    pub fn asset_id(&self) -> Option<&str> {
        match self {
            Self::Speech { audio } => audio.as_ref().map(|a| a.asset_id.as_str()),
            Self::Material { asset_id, .. } | Self::Video { asset_id, .. } => Some(asset_id),
        }
    }
}
