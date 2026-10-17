use super::{AudioQuery, VoicevoxResponse};
use sungeki_media::{ExpectedAudio, VerifiedWav, WavError, verify_wav};

pub struct VerifiedVoicevoxResponse {
    pub query: AudioQuery,
    pub audio: VerifiedWav,
}
impl VoicevoxResponse {
    /// Consume the received bytes and validate them against the sent query.
    /// Run on a blocking/media worker; this does not persist or edit a Project.
    pub fn verify(self) -> Result<VerifiedVoicevoxResponse, WavError> {
        let audio = verify_wav(
            self.unverified_wav,
            ExpectedAudio {
                sample_rate: self.query.output_sampling_rate,
                channels: if self.query.output_stereo { 2 } else { 1 },
            },
        )?;
        Ok(VerifiedVoicevoxResponse {
            query: self.query,
            audio,
        })
    }
}
