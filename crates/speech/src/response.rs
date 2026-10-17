//! Engine-neutral handoff to a blocking WAV validation and storage worker.
use crate::irodori::PreparedIrodoriInput;
use crate::{
    InputError, qwen::PreparedQwenInput, sbv2::PreparedSbv2Input, voicevox::VoicevoxResponse,
};
use sungeki_media::{VerifiedWav, WavError};

pub struct SpeechResponse(Received);
enum Received {
    Irodori {
        input: Box<PreparedIrodoriInput>,
        bytes: Vec<u8>,
    },
    Voicevox(VoicevoxResponse),
    Qwen {
        input: Box<PreparedQwenInput>,
        bytes: Vec<u8>,
    },
    Sbv2 {
        input: Box<PreparedSbv2Input>,
        bytes: Vec<u8>,
    },
}
impl From<VoicevoxResponse> for SpeechResponse {
    fn from(response: VoicevoxResponse) -> Self {
        Self(Received::Voicevox(response))
    }
}
impl SpeechResponse {
    pub fn irodori_input_fingerprint(&self) -> Option<&str> {
        match &self.0 {
            Received::Irodori { input, .. } => Some(input.fingerprint()),
            _ => None,
        }
    }
    pub fn irodori(input: &PreparedIrodoriInput, bytes: Vec<u8>) -> Result<Self, InputError> {
        if input.selection().is_none() {
            return Err(InputError::Invalid(
                "Irodori応答には選択済み入力が必要です".into(),
            ));
        }
        Ok(Self(Received::Irodori {
            input: Box::new(input.clone()),
            bytes,
        }))
    }

    pub fn qwen_input_fingerprint(&self) -> Option<&str> {
        match &self.0 {
            Received::Qwen { input, .. } => Some(input.fingerprint()),
            _ => None,
        }
    }
    pub fn qwen(input: &PreparedQwenInput, bytes: Vec<u8>) -> Result<Self, InputError> {
        if input.identity().is_none() {
            return Err(InputError::Invalid(
                "Qwen応答にはモデル識別済み入力が必要です".into(),
            ));
        }
        Ok(Self(Received::Qwen {
            input: Box::new(input.clone()),
            bytes,
        }))
    }

    pub fn sbv2_input_fingerprint(&self) -> Option<&str> {
        match &self.0 {
            Received::Voicevox(_) | Received::Qwen { .. } | Received::Irodori { .. } => None,
            Received::Sbv2 { input, .. } => Some(input.fingerprint()),
        }
    }
    /// Bind the bytes to the dispatched input, never infer expected format from WAV.
    /// The caller still owns engine attestation, cancellation and job/input matching.
    pub fn sbv2(input: &PreparedSbv2Input, bytes: Vec<u8>) -> Result<Self, InputError> {
        if input.identity().is_none() || input.model_config_sha256().is_none() {
            return Err(InputError::Invalid(
                "SBV2応答にはモデル識別済み入力が必要です".into(),
            ));
        }
        Ok(Self(Received::Sbv2 {
            input: Box::new(input.clone()),
            bytes,
        }))
    }
    /// Must run on a blocking/media worker. Success is not a Project edit or job success.
    pub fn verify(self) -> Result<VerifiedWav, WavError> {
        match self.0 {
            Received::Irodori { input, bytes } => input.verify_audio(bytes),
            Received::Voicevox(response) => response.verify().map(|verified| verified.audio),
            Received::Sbv2 { input, bytes } => input.verify_audio(bytes),
            Received::Qwen { input, bytes } => input.verify_audio(bytes),
        }
    }
}
