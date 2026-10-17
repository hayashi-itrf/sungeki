//! Declared runtime originals. Only an owned host can verify these against files.
use super::{Error, PreparedQwenInput, RuntimeIdentity};
use sha2::{Digest, Sha256};
impl PreparedQwenInput {
    /// Bind once after the host inspects originals and engine configuration.
    /// This method checks consistency, not the existence/content of any file.
    pub fn bind_identity(mut self, identity: &RuntimeIdentity) -> Result<Self, Error> {
        identity.validate().map_err(|_| Error::Configuration)?;
        if self.identity.is_some()
            || identity.backend != self.selection.backend
            || identity.model_id != self.selection.model_id
            || identity.sample_rate != self.expected.sample_rate
            || identity.channels != self.expected.channels
        {
            return Err(Error::Configuration);
        }
        let bytes =
            serde_json::to_vec(&("vvm-qwen-identified-input-v1", &self.fingerprint, identity))
                .map_err(|_| Error::Configuration)?;
        self.fingerprint = format!("{:x}", Sha256::digest(bytes));
        self.identity = Some(identity.clone());
        Ok(self)
    }
    pub fn identity(&self) -> Option<&RuntimeIdentity> {
        self.identity.as_ref()
    }
}
