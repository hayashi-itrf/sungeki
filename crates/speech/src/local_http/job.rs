use super::PreparedLocalInput;
use crate::InputError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sungeki_project::{LipSyncSettings, Project};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "Wire")]
pub struct LocalSpeechJobInput {
    capture: Capture,
    input_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Capture {
    local_http_version: u32,
    source_revision: u32,
    project: Project,
    clip_id: String,
    endpoint: String,
    engine_version: String,
    parent_job_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    capture: Capture,
    input_hash: String,
}
fn invalid(e: impl ToString) -> InputError {
    InputError::Invalid(e.to_string())
}
impl LocalSpeechJobInput {
    pub fn new(
        revision: u32,
        project: &Project,
        clip: &str,
        endpoint: &str,
        version: &str,
    ) -> Result<Self, InputError> {
        Self::checked(Capture {
            local_http_version: 1,
            source_revision: revision,
            project: project.clone(),
            clip_id: clip.into(),
            endpoint: crate::voicevox::canonical_endpoint(endpoint).map_err(invalid)?,
            engine_version: version.into(),
            parent_job_id: None,
        })
    }
    fn checked(capture: Capture) -> Result<Self, InputError> {
        if capture.local_http_version != 1
            || crate::voicevox::canonical_endpoint(&capture.endpoint).map_err(invalid)?
                != capture.endpoint
        {
            return Err(invalid("ローカル音声入力が不正です"));
        }
        if let Some(parent) = &capture.parent_job_id {
            if parent.len() != 36
                || parent.bytes().enumerate().any(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        b != b'-'
                    } else {
                        !b.is_ascii_hexdigit()
                    }
                })
            {
                return Err(invalid("制作親IDが不正です"));
            }
        }
        let bytes = serde_json::to_vec(&capture).map_err(invalid)?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(invalid("音声入力は16MiB以内です"));
        }
        let input = Self {
            capture,
            input_hash: format!("{:x}", Sha256::digest(bytes)),
        };
        input.prepared()?;
        Ok(input)
    }
    pub fn prepared(&self) -> Result<PreparedLocalInput, InputError> {
        PreparedLocalInput::new(
            &self.capture.project,
            &self.capture.clip_id,
            &self.capture.engine_version,
        )
    }
    pub fn matches_project(&self, p: &Project) -> Result<bool, InputError> {
        Ok(self.prepared()?.matches(p))
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
    pub fn endpoint(&self) -> &str {
        &self.capture.endpoint
    }
    pub fn engine_version(&self) -> &str {
        &self.capture.engine_version
    }
    pub fn source_revision(&self) -> u32 {
        self.capture.source_revision
    }
    pub fn parent_job_id(&self) -> Option<&str> {
        self.capture.parent_job_id.as_deref()
    }
    pub fn lip_sync_settings(&self) -> LipSyncSettings {
        Default::default()
    }
    pub fn for_parent(self, parent: &str) -> Result<Self, InputError> {
        let mut c = self.capture;
        if c.parent_job_id.as_deref().is_some_and(|p| p != parent) {
            return Err(invalid("制作親は変更できません"));
        }
        c.parent_job_id = Some(parent.into());
        Self::checked(c)
    }
    pub fn recapture_for_reuse(
        &self,
        revision: u32,
        project: &Project,
        clip: &str,
        parent: &str,
    ) -> Result<Self, InputError> {
        Self::new(
            revision,
            project,
            clip,
            self.endpoint(),
            self.engine_version(),
        )?
        .for_parent(parent)
    }
}
impl TryFrom<Wire> for LocalSpeechJobInput {
    type Error = InputError;
    fn try_from(w: Wire) -> Result<Self, InputError> {
        let i = Self::checked(w.capture)?;
        if i.input_hash != w.input_hash {
            return Err(invalid("音声入力のhashが一致しません"));
        }
        Ok(i)
    }
}
