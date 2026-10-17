//! Immutable durable input. Deserialization verifies the complete captured input
//! and its hash, not just the syntactic shape of stored JSON.
use crate::{InputError, PreparedSpeechClip, VoicevoxIdentity, voicevox::canonical_endpoint};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sungeki_project::Project;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "Wire")]
pub struct SpeechJobInput {
    format_version: u32,
    lip_sync_settings: sungeki_project::LipSyncSettings,
    lip_sync_version: u32,
    source_revision: u32,
    project: Project,
    clip_id: String,
    endpoint: String,
    identity: VoicevoxIdentity,
    input_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_job_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format_version: u32,
    lip_sync_settings: sungeki_project::LipSyncSettings,
    lip_sync_version: u32,
    source_revision: u32,
    project: Project,
    clip_id: String,
    endpoint: String,
    identity: VoicevoxIdentity,
    input_hash: String,
    #[serde(default)]
    parent_job_id: Option<String>,
}
impl SpeechJobInput {
    /// Identity must come from actual discovery and be verified again under the
    /// engine lease before dispatch. URL or stored identity alone is no attestation.
    pub fn new(
        source_revision: u32,
        project: &Project,
        clip_id: &str,
        endpoint: &str,
        identity: &VoicevoxIdentity,
    ) -> Result<Self, InputError> {
        let prepared = PreparedSpeechClip::new(project, clip_id, identity)?;
        let endpoint =
            canonical_endpoint(endpoint).map_err(|e| InputError::Invalid(e.to_string()))?;
        // Full project capture protects scene/target context. Audio fingerprint
        // remains separate so unrelated edits do not invalidate result application.
        let encoded = serde_json::to_vec(&(
            "vvm-speech-job-input-v2",
            sungeki_project::LipSyncSettings::default(),
            sungeki_project::LIPSYNC_ANALYSIS_VERSION,
            source_revision,
            project,
            clip_id,
            &endpoint,
            identity,
            prepared.audio().fingerprint(),
        ))
        .map_err(|e| InputError::Invalid(e.to_string()))?;
        if encoded.len() > 16 * 1024 * 1024 {
            return Err(InputError::Invalid("音声ジョブ入力は16MiBまでです".into()));
        }
        Ok(Self {
            parent_job_id: None,
            format_version: 2,
            lip_sync_settings: Default::default(),
            lip_sync_version: sungeki_project::LIPSYNC_ANALYSIS_VERSION,
            source_revision,
            project: project.clone(),
            clip_id: clip_id.into(),
            endpoint,
            identity: identity.clone(),
            input_hash: format!("{:x}", Sha256::digest(encoded)),
        })
    }
    /// Bind result application to a durable production parent. This changes the
    /// input digest; removing or replacing the parent invalidates stored input.
    pub fn for_parent(mut self, parent: &str) -> Result<Self, InputError> {
        if self.parent_job_id.as_deref() == Some(parent) {
            return Ok(self);
        }
        if self.parent_job_id.is_some()
            || parent.len() != 36
            || parent.bytes().enumerate().any(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b != b'-'
                } else {
                    !b.is_ascii_digit() && !(b'a'..=b'f').contains(&b)
                }
            })
        {
            return Err(InputError::Invalid("制作親IDが不正です".into()));
        }
        let bytes = serde_json::to_vec(&("vvm-speech-build-parent-v1", &self.input_hash, parent))
            .map_err(|e| InputError::Invalid(e.to_string()))?;
        self.input_hash = format!("{:x}", Sha256::digest(bytes));
        self.parent_job_id = Some(parent.into());
        Ok(self)
    }
    /// Construct a local reuse candidate; stored identity is not fresh discovery.
    pub fn recapture_for_reuse(
        &self,
        revision: u32,
        project: &Project,
        clip_id: &str,
        parent_id: &str,
    ) -> Result<Self, InputError> {
        Self::new(revision, project, clip_id, &self.endpoint, &self.identity)?.for_parent(parent_id)
    }
    pub fn parent_job_id(&self) -> Option<&str> {
        self.parent_job_id.as_deref()
    }
    pub fn lip_sync_settings(&self) -> sungeki_project::LipSyncSettings {
        self.lip_sync_settings
    }
    pub fn source_revision(&self) -> u32 {
        self.source_revision
    }
    pub fn project(&self) -> &Project {
        &self.project
    }
    pub fn clip_id(&self) -> &str {
        &self.clip_id
    }
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    pub fn identity(&self) -> &VoicevoxIdentity {
        &self.identity
    }
    pub fn input_hash(&self) -> &str {
        &self.input_hash
    }
    pub fn prepared(&self) -> Result<PreparedSpeechClip, InputError> {
        PreparedSpeechClip::new(&self.project, &self.clip_id, &self.identity)
    }
}
impl TryFrom<Wire> for SpeechJobInput {
    type Error = InputError;
    fn try_from(wire: Wire) -> Result<Self, Self::Error> {
        if wire.format_version != 2 {
            return Err(InputError::Invalid("未対応の音声ジョブ入力形式です".into()));
        }
        let input = Self::new(
            wire.source_revision,
            &wire.project,
            &wire.clip_id,
            &wire.endpoint,
            &wire.identity,
        )?;
        let input = if let Some(parent) = wire.parent_job_id {
            input.for_parent(&parent)?
        } else {
            input
        };
        if input.input_hash != wire.input_hash
            || input.endpoint != wire.endpoint
            || input.lip_sync_settings != wire.lip_sync_settings
            || input.lip_sync_version != wire.lip_sync_version
        {
            return Err(InputError::Invalid(
                "保存された音声ジョブ入力のhashまたは接続先が一致しません".into(),
            ));
        }
        Ok(input)
    }
}
