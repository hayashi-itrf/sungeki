//! VOICEVOX input validation and bounded loopback HTTP transport.
mod catalog;
mod speaker_details;
pub use speaker_details::SpeakerDetails;
mod http;
mod query;
pub use catalog::{EngineInfo, Speaker, SpeakerStyle, VoiceCatalog, VoicevoxDiscovery};
mod verified;
pub use verified::VerifiedVoicevoxResponse;
mod transport;
mod transport_error;
pub use query::{AccentPhrase, AudioQuery, Mora, prepare_synthesis_query};
pub use sungeki_project::VoicevoxSettings;
pub use transport::{VoicevoxClient, VoicevoxResponse};
pub use transport_error::{Stage, TransportError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoicevoxError(pub String);
impl std::fmt::Display for VoicevoxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for VoicevoxError {}

pub(super) fn finite(name: &str, value: f64, minimum: Option<f64>) -> Result<(), VoicevoxError> {
    if !value.is_finite() || minimum.is_some_and(|min| value < min) {
        return Err(VoicevoxError(format!("{name}の値が不正です")));
    }
    Ok(())
}

impl From<sungeki_project::ValidationError> for VoicevoxError {
    fn from(error: sungeki_project::ValidationError) -> Self {
        Self(error.to_string())
    }
}

/// Canonical loopback endpoint used by persisted jobs; does not connect.
pub fn canonical_endpoint(base: &str) -> Result<String, TransportError> {
    Ok(http::endpoint(base)?.to_string())
}
