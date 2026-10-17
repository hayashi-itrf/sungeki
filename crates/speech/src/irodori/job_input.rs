//! Durable capture; loading verifies inputs, but does not attest a running engine.
use super::{Catalog, ModelCapabilities, PreparedIrodoriClip, RegisteredVoice};
use crate::InputError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sungeki_project::{IrodoriIdentity, LIPSYNC_ANALYSIS_VERSION, LipSyncSettings, Project};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "Wire")]
pub struct IrodoriJobInput {
    capture: Capture,
    input_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Capture {
    format_version: u32,
    source_revision: u32,
    project: Project,
    clip_id: String,
    identity: IrodoriIdentity,
    model: ModelCapabilities,
    voices: Vec<RegisteredVoice>,
    lip_sync_settings: LipSyncSettings,
    lip_sync_version: u32,
    parent_job_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    profile_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    capture: Capture,
    input_hash: String,
}
fn invalid(message: impl ToString) -> InputError {
    InputError::Invalid(message.to_string())
}
impl IrodoriJobInput {
    /// Caller must inspect the originals under its engine lease again before use.
    pub fn new(
        source_revision: u32,
        project: &Project,
        clip_id: &str,
        identity: &IrodoriIdentity,
        catalog: &Catalog,
    ) -> Result<Self, InputError> {
        let capture = Capture {
            format_version: 1,
            source_revision,
            project: project.clone(),
            clip_id: clip_id.into(),
            identity: identity.clone(),
            model: catalog.model().clone(),
            voices: catalog.voices().cloned().collect(),
            lip_sync_settings: Default::default(),
            lip_sync_version: LIPSYNC_ANALYSIS_VERSION,
            parent_job_id: None,
            profile_id: None,
        };
        Self::checked(capture)
    }
    fn checked(capture: Capture) -> Result<Self, InputError> {
        if capture.format_version != 1
            || capture.lip_sync_version != LIPSYNC_ANALYSIS_VERSION
            || capture.lip_sync_settings != LipSyncSettings::default()
        {
            return Err(invalid("未対応または不正なIrodoriジョブ入力です"));
        }
        if let Some(profile) = &capture.profile_id {
            validate_profile(profile)?;
        }
        if let Some(parent) = &capture.parent_job_id {
            validate_parent(parent)?;
        }
        let bytes = serde_json::to_vec(&capture).map_err(invalid)?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(invalid("音声ジョブ入力は16MiBまでです"));
        }
        let mut input = Self {
            capture,
            input_hash: String::new(),
        };
        let prepared = input.prepared()?;
        let hash_bytes = serde_json::to_vec(&(
            "vvm-irodori-job-input-v1",
            &input.capture,
            prepared.audio().fingerprint(),
        ))
        .map_err(invalid)?;
        input.input_hash = format!("{:x}", Sha256::digest(hash_bytes));
        Ok(input)
    }
    pub fn catalog(&self) -> Result<Catalog, InputError> {
        Catalog::new(self.capture.model.clone(), self.capture.voices.clone())
    }
    pub fn prepared(&self) -> Result<PreparedIrodoriClip, InputError> {
        self.prepare_project(&self.capture.project)
    }
    fn prepare_project(&self, project: &Project) -> Result<PreparedIrodoriClip, InputError> {
        let catalog = self.catalog()?;
        PreparedIrodoriClip::new(
            project,
            &self.capture.clip_id,
            &catalog,
            &self.capture.identity,
        )
    }
    /// Compare only Project edits against the captured environment. The worker must
    /// separately attest current model/process identity before accepting its response.
    pub fn matches_project(&self, project: &Project) -> Result<bool, InputError> {
        let catalog = self.catalog()?;
        Ok(self
            .prepared()?
            .matches(project, &catalog, &self.capture.identity))
    }
    /// Construct a new local reuse candidate from the captured catalog/identity.
    /// This is not discovery or permission to dispatch an engine request.
    pub fn recapture_for_reuse(
        &self,
        revision: u32,
        project: &Project,
        clip_id: &str,
        parent_id: &str,
    ) -> Result<Self, InputError> {
        validate_parent(parent_id)?;
        let mut capture = self.capture.clone();
        capture.source_revision = revision;
        capture.project = project.clone();
        capture.clip_id = clip_id.into();
        capture.parent_job_id = Some(parent_id.into());
        Self::checked(capture)
    }
    pub fn for_parent(self, parent: &str) -> Result<Self, InputError> {
        validate_parent(parent)?;
        if let Some(previous) = &self.capture.parent_job_id {
            return if previous == parent {
                Ok(self)
            } else {
                Err(invalid("制作親IDは変更できません"))
            };
        }
        let mut capture = self.capture;
        capture.parent_job_id = Some(parent.into());
        Self::checked(capture)
    }
    pub fn for_profile(self, profile: &str) -> Result<Self, InputError> {
        validate_profile(profile)?;
        if let Some(previous) = &self.capture.profile_id {
            return if previous == profile {
                Ok(self)
            } else {
                Err(invalid("Irodori設定IDは変更できません"))
            };
        }
        let mut capture = self.capture;
        capture.profile_id = Some(profile.into());
        Self::checked(capture)
    }
    pub fn profile_id(&self) -> Option<&str> {
        self.capture.profile_id.as_deref()
    }
    pub fn input_hash(&self) -> &str {
        &self.input_hash
    }
    pub fn project(&self) -> &Project {
        &self.capture.project
    }
    pub fn clip_id(&self) -> &str {
        &self.capture.clip_id
    }
    pub fn source_revision(&self) -> u32 {
        self.capture.source_revision
    }
    pub fn identity(&self) -> &IrodoriIdentity {
        &self.capture.identity
    }
    pub fn parent_job_id(&self) -> Option<&str> {
        self.capture.parent_job_id.as_deref()
    }
    pub fn lip_sync_settings(&self) -> LipSyncSettings {
        self.capture.lip_sync_settings
    }
}
fn validate_parent(parent: &str) -> Result<(), InputError> {
    if parent.len() != 36
        || parent.bytes().enumerate().any(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b != b'-'
            } else {
                !b.is_ascii_digit() && !(b'a'..=b'f').contains(&b)
            }
        })
    {
        return Err(invalid("制作親IDが不正です"));
    }
    Ok(())
}
impl TryFrom<Wire> for IrodoriJobInput {
    type Error = InputError;
    fn try_from(wire: Wire) -> Result<Self, Self::Error> {
        let checked = Self::checked(wire.capture)?;
        if checked.input_hash != wire.input_hash {
            return Err(invalid("保存されたIrodoriジョブ入力のhashが一致しません"));
        }
        Ok(checked)
    }
}

fn validate_profile(id: &str) -> Result<(), InputError> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(invalid("Irodori設定IDが不正です"));
    }
    Ok(())
}
