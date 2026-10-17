//! Read an explicitly supplied model config; never follow server paths or download models.
use super::{Catalog, Error, Language, Settings};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use sungeki_media::ExpectedAudio;
#[derive(Deserialize)]
struct Config {
    version: String,
    data: Data,
}
#[derive(Deserialize)]
struct Data {
    sampling_rate: u32,
    spk2id: BTreeMap<String, u32>,
    num_styles: usize,
    style2id: BTreeMap<String, u32>,
}
#[derive(Debug, Clone)]
pub struct ModelConfig {
    version: String,
    sample_rate: u32,
    speakers: BTreeMap<String, u32>,
    styles: BTreeMap<String, u32>,
    sha256: String,
}
impl ModelConfig {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(Error::TooLarge);
        }
        let c: Config = serde_json::from_slice(bytes).map_err(|_| Error::InvalidResponse)?;
        if c.version.trim().is_empty()
            || c.version.len() > 128
            || c.version.chars().any(char::is_control)
            || !(8000..=192000).contains(&c.data.sampling_rate)
            || !super::catalog::names(&c.data.spk2id)
            || !super::catalog::names(&c.data.style2id)
            || c.data.num_styles != c.data.style2id.len()
            || c.data
                .style2id
                .values()
                .any(|id| *id as usize >= c.data.num_styles)
        {
            return Err(Error::InvalidResponse);
        }
        Ok(Self {
            version: c.version,
            sample_rate: c.data.sampling_rate,
            speakers: c.data.spk2id,
            styles: c.data.style2id,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        })
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn expected_audio(
        &self,
        catalog: &Catalog,
        settings: &Settings,
    ) -> Result<ExpectedAudio, Error> {
        catalog.select(settings)?;
        let model = &catalog.models()[&settings.model_id];
        if model.spk2id != self.speakers || model.style2id != self.styles {
            return Err(Error::ModelChanged);
        }
        if self.version.ends_with("JP-Extra") && !matches!(settings.language, Language::JP) {
            return Err(Error::Configuration);
        }
        // Installed server returns the mono waveform from TTSModel.infer with this rate.
        Ok(ExpectedAudio {
            sample_rate: self.sample_rate,
            channels: 1,
        })
    }
}
