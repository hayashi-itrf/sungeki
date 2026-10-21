//! Current Sungeki Maker durable input formats; all validate themselves on deserialize.
use crate::{sessions::Sessions, speech_state::invalid};
use serde::{Deserialize, Serialize};
use sungeki_jobs::Resource;
use sungeki_project::{LipSyncSettings, Project};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::{
    SpeechJobInput, irodori::IrodoriJobInput, qwen::QwenJobInput, sbv2::Sbv2JobInput,
};

// Strict engine-specific captures validate their structure and hash on load.
// Keep existing Sungeki Maker job bytes intact; this does not read the old application's jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Input {
    Local(sungeki_speech::local_http::LocalSpeechJobInput),
    Voicevox(SpeechJobInput),
    Sbv2(Sbv2JobInput),
    Qwen(QwenJobInput),
    Irodori(IrodoriJobInput),
}
pub(crate) struct ApplicationInput {
    pub irodori_source: Option<sungeki_project::IrodoriAudioSource>,
    pub matches: bool,
    pub credit: String,
    pub fingerprint: String,
    pub engine_version: String,
    pub qwen_source: Option<sungeki_project::QwenAudioSource>,
    pub sbv2_source: Option<sungeki_project::Sbv2AudioSource>,
}
impl Input {
    pub fn applied_audio_matches(&self, project: &Project) -> bool {
        sungeki_speech::audio_reference_matches(project, self.clip_id()).unwrap_or(false)
    }
    pub fn accepts_response(
        &self,
        response: &sungeki_speech::SpeechResponse,
    ) -> Result<bool, ApiError> {
        Ok(match self {
            Self::Local(i) => {
                response.local_input_fingerprint()
                    == Some(i.prepared().map_err(invalid)?.fingerprint())
            }
            Self::Voicevox(_) => {
                response.local_input_fingerprint().is_none()
                    && response.irodori_input_fingerprint().is_none()
                    && response.sbv2_input_fingerprint().is_none()
                    && response.qwen_input_fingerprint().is_none()
            }
            Self::Irodori(i) => {
                response.irodori_input_fingerprint()
                    == Some(i.prepared().map_err(invalid)?.audio().fingerprint())
            }
            Self::Qwen(i) => {
                response.qwen_input_fingerprint()
                    == Some(i.prepared().map_err(invalid)?.audio().fingerprint())
            }
            Self::Sbv2(i) => {
                response.sbv2_input_fingerprint()
                    == Some(i.prepared().map_err(invalid)?.audio().fingerprint())
            }
        })
    }
    pub fn clip_id(&self) -> &str {
        match self {
            Self::Local(i) => i.clip_id(),
            Self::Voicevox(i) => i.clip_id(),
            Self::Sbv2(i) => i.clip_id(),
            Self::Qwen(i) => i.clip_id(),
            Self::Irodori(i) => i.clip_id(),
        }
    }
    pub fn parent_job_id(&self) -> Option<&str> {
        match self {
            Self::Local(i) => i.parent_job_id(),
            Self::Voicevox(i) => i.parent_job_id(),
            Self::Sbv2(i) => i.parent_job_id(),
            Self::Qwen(i) => i.parent_job_id(),
            Self::Irodori(i) => i.parent_job_id(),
        }
    }
    pub fn application(&self, current: &Project) -> Result<ApplicationInput, ApiError> {
        Ok(match self {
            Self::Local(i) => {
                let p = i.prepared().map_err(invalid)?;
                ApplicationInput {
                    irodori_source: None,
                    qwen_source: None,
                    sbv2_source: None,
                    matches: p.matches(current),
                    credit: p.character().credit.clone(),
                    fingerprint: p.fingerprint().into(),
                    engine_version: i.engine_version().into(),
                }
            }
            Self::Voicevox(i) => {
                let prepared = i.prepared().map_err(invalid)?;
                ApplicationInput {
                    irodori_source: None,
                    matches: prepared.matches(current, i.identity()),
                    credit: prepared.character().credit.clone(),
                    fingerprint: prepared.audio().fingerprint().into(),
                    engine_version: i.identity().engine_version.clone(),
                    qwen_source: None,
                    sbv2_source: None,
                }
            }
            Self::Qwen(i) => {
                let prepared = i.prepared().map_err(invalid)?;
                ApplicationInput {
                    irodori_source: None,
                    matches: i.matches_project(current).map_err(invalid)?,
                    credit: prepared.character().credit.clone(),
                    fingerprint: prepared.audio().fingerprint().into(),
                    engine_version: i.identity().engine_manifest_sha256.clone(),
                    qwen_source: Some(prepared.audio().audio_source().map_err(invalid)?),
                    sbv2_source: None,
                }
            }
            Self::Irodori(i) => {
                let prepared = i.prepared().map_err(invalid)?;
                ApplicationInput {
                    irodori_source: Some(prepared.audio().audio_source().clone()),
                    matches: i.matches_project(current).map_err(invalid)?,
                    credit: prepared.character().credit.clone(),
                    fingerprint: prepared.audio().fingerprint().into(),
                    engine_version: i.identity().engine_manifest_sha256.clone(),
                    qwen_source: None,
                    sbv2_source: None,
                }
            }
            Self::Sbv2(i) => {
                let prepared = i.prepared().map_err(invalid)?;
                ApplicationInput {
                    irodori_source: None,
                    matches: i.matches_project(current).map_err(invalid)?,
                    credit: prepared.character().credit.clone(),
                    fingerprint: prepared.audio().fingerprint().into(),
                    engine_version: i.identity().engine_revision.clone(),
                    qwen_source: None,
                    sbv2_source: Some(prepared.audio().audio_source().map_err(invalid)?),
                }
            }
        })
    }
    pub fn voicevox(&self) -> Result<&SpeechJobInput, ApiError> {
        match self {
            Self::Voicevox(input) => Ok(input),
            Self::Local(_) | Self::Sbv2(_) | Self::Qwen(_) | Self::Irodori(_) => {
                Err(ApiError::new(
                    ErrorCode::CapabilityUnavailable,
                    "この操作はVOICEVOX専用です",
                ))
            }
        }
    }
    pub fn project(&self) -> &Project {
        match self {
            Self::Local(i) => i.project(),
            Self::Voicevox(i) => i.project(),
            Self::Sbv2(i) => i.project(),
            Self::Qwen(i) => i.project(),
            Self::Irodori(i) => i.project(),
        }
    }
    pub fn source_revision(&self) -> u32 {
        match self {
            Self::Local(i) => i.source_revision(),
            Self::Voicevox(i) => i.source_revision(),
            Self::Sbv2(i) => i.source_revision(),
            Self::Qwen(i) => i.source_revision(),
            Self::Irodori(i) => i.source_revision(),
        }
    }
    pub fn input_hash(&self) -> &str {
        match self {
            Self::Local(i) => i.input_hash(),
            Self::Voicevox(i) => i.input_hash(),
            Self::Sbv2(i) => i.input_hash(),
            Self::Qwen(i) => i.input_hash(),
            Self::Irodori(i) => i.input_hash(),
        }
    }
    pub fn lip_sync_settings(&self) -> LipSyncSettings {
        match self {
            Self::Local(i) => i.lip_sync_settings(),
            Self::Voicevox(i) => i.lip_sync_settings(),
            Self::Sbv2(i) => i.lip_sync_settings(),
            Self::Qwen(i) => i.lip_sync_settings(),
            Self::Irodori(i) => i.lip_sync_settings(),
        }
    }
    pub fn resources(&self) -> Vec<Resource> {
        let key = match self {
            Self::Local(_) => "local-http-speech".into(),
            Self::Voicevox(i) => i.identity().engine_id.clone(),
            // Serialize every SBV2 endpoint/model until owned process bindings exist.
            // A changed URL or model hash must never bypass unsettled synthesis.
            Self::Sbv2(_) => "style-bert-vits2".into(),
            Self::Qwen(_) => "qwen3-tts".into(),
            Self::Irodori(_) => "irodori".into(),
        };
        vec![Resource::Engine(key), Resource::Gpu]
    }
    pub fn validate_parent(
        &self,
        sessions: &Sessions,
        id: &str,
        sid: &str,
    ) -> Result<(), ApiError> {
        crate::build_record::validate_speech_parent(sessions, self, id, sid)
    }
}
