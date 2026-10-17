//! Server metadata selection, not a hash of model weights or cloned voice files.
use super::{Error, QwenClient, Settings, cancelled, failure, read_response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
};
pub use sungeki_project::QwenSelection as Selection;
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Voice {
    id: String,
    name: String,
    language: Option<String>,
    description: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
struct Backend {
    name: String,
    model_id: String,
    ready: bool,
}
#[derive(Debug, Serialize, Deserialize)]
struct Health {
    status: String,
    backend: Backend,
    version: String,
}
#[derive(Deserialize)]
struct Model {
    id: String,
}
#[derive(Deserialize)]
struct Models {
    data: Vec<Model>,
}
#[derive(Deserialize)]
struct Voices {
    voices: Vec<Voice>,
    languages: Vec<String>,
}
#[derive(Debug)]
pub struct Catalog {
    health: Health,
    models: BTreeSet<String>,
    voices: BTreeMap<String, Voice>,
    languages: BTreeSet<String>,
}
fn valid(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}
impl Catalog {
    pub fn voices(&self) -> impl Iterator<Item = (&str, &str)> {
        self.voices
            .values()
            .map(|voice| (voice.id.as_str(), voice.name.as_str()))
    }
    pub fn languages(&self) -> impl Iterator<Item = &str> {
        self.languages.iter().map(String::as_str)
    }
    /// Canonical metadata capture only; no claim about current runtime originals.
    pub fn snapshot(&self) -> Result<[serde_json::Value; 3], Error> {
        Ok([
            serde_json::to_value(&self.health).map_err(|_| Error::InvalidResponse)?,
            serde_json::json!({"data":self.models.iter().map(|id|serde_json::json!({"id":id})).collect::<Vec<_>>()}),
            serde_json::json!({"voices":self.voices.values().collect::<Vec<_>>(),"languages":self.languages}),
        ])
    }

    pub fn parse(
        health: serde_json::Value,
        models: serde_json::Value,
        voices: serde_json::Value,
    ) -> Result<Self, Error> {
        let health: Health = serde_json::from_value(health).map_err(|_| Error::InvalidResponse)?;
        if health.status != "healthy" || !health.backend.ready {
            return Err(Error::NotReady);
        }
        if !valid(&health.backend.name, 128)
            || !valid(&health.backend.model_id, 4096)
            || !valid(&health.version, 128)
        {
            return Err(Error::InvalidResponse);
        }
        let models: Models = serde_json::from_value(models).map_err(|_| Error::InvalidResponse)?;
        let voices: Voices = serde_json::from_value(voices).map_err(|_| Error::InvalidResponse)?;
        if models.data.is_empty()
            || models.data.len() > 4096
            || voices.voices.is_empty()
            || voices.voices.len() > 4096
            || voices.languages.is_empty()
            || voices.languages.len() > 256
        {
            return Err(Error::InvalidResponse);
        }
        let mut result = Self {
            health,
            models: BTreeSet::new(),
            voices: BTreeMap::new(),
            languages: BTreeSet::new(),
        };
        for model in models.data {
            if !valid(&model.id, 128) || !result.models.insert(model.id) {
                return Err(Error::InvalidResponse);
            }
        }
        for voice in voices.voices {
            if !valid(&voice.id, 128)
                || !valid(&voice.name, 1024)
                || voice.language.as_ref().is_some_and(|s| !valid(s, 128))
                || voice.description.as_ref().is_some_and(|s| !valid(s, 4096))
                || result.voices.insert(voice.id.clone(), voice).is_some()
            {
                return Err(Error::InvalidResponse);
            }
        }
        for language in voices.languages {
            if !valid(&language, 128) || !result.languages.insert(language) {
                return Err(Error::InvalidResponse);
            }
        }
        Ok(result)
    }
    pub fn select(&self, settings: &Settings) -> Result<Selection, Error> {
        settings.validate().map_err(|_| Error::Configuration)?;
        let voice = self
            .voices
            .get(&settings.voice)
            .ok_or(Error::Configuration)?;
        if !self.models.contains(&settings.model)
            || (settings.language != "Auto" && !self.languages.contains(&settings.language))
        {
            return Err(Error::Configuration);
        }
        let bytes = serde_json::to_vec(&(
            "vvm-qwen-server-metadata-v1",
            &self.health.backend.name,
            &self.health.backend.model_id,
            &self.health.version,
            &settings.model,
            voice,
            &settings.language,
        ))
        .map_err(|_| Error::InvalidResponse)?;
        Ok(Selection {
            backend: self.health.backend.name.clone(),
            model_id: self.health.backend.model_id.clone(),
            request_model: settings.model.clone(),
            voice: settings.voice.clone(),
            language: settings.language.clone(),
            metadata_sha256: format!("{:x}", Sha256::digest(bytes)),
        })
    }
}
impl QwenClient {
    async fn metadata(&self, path: &str, cancel: &AtomicBool) -> Result<serde_json::Value, Error> {
        cancelled(cancel)?;
        let mut url = self.base.clone();
        url.set_path(path);
        let response = self.client.get(url).send().await.map_err(failure)?;
        let bytes = read_response(response, 1024 * 1024, cancel).await?;
        serde_json::from_slice(&bytes).map_err(|_| Error::InvalidResponse)
    }
    pub async fn catalog(&self, cancel: &AtomicBool) -> Result<Catalog, Error> {
        let health = self.metadata("health", cancel).await?;
        let models = self.metadata("v1/models", cancel).await?;
        let voices = self.metadata("v1/audio/voices", cancel).await?;
        let after = self.metadata("health", cancel).await?;
        let before = Catalog::parse(health, models.clone(), voices.clone())?;
        let after = Catalog::parse(after, models, voices)?;
        if before.health.backend.name != after.health.backend.name
            || before.health.backend.model_id != after.health.backend.model_id
            || before.health.version != after.health.version
        {
            return Err(Error::ModelChanged);
        }
        Ok(after)
    }
}
