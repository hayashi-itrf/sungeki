use super::{Catalog, ModelCapabilities, RegisteredVoice};
use reqwest::{Client, Url};
use serde::Deserialize;
use std::{
    net::Ipv4Addr,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use sungeki_project::IrodoriSettings as Settings;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Configuration,
    Cancelled,
    Connection,
    Timeout,
    Http(u16),
    TooLarge,
    InvalidResponse,
    NotReady,
    ModelChanged,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Irodori: {self:?}")
    }
}
impl std::error::Error for Error {}

pub(super) fn cancelled(flag: &AtomicBool) -> Result<(), Error> {
    if flag.load(Ordering::Acquire) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
fn failure(e: reqwest::Error) -> Error {
    // Never include speech, instructions, URLs or server error bodies.
    if e.is_timeout() {
        Error::Timeout
    } else {
        Error::Connection
    }
}
pub fn canonical_endpoint(base: &str) -> Result<String, Error> {
    let url = Url::parse(base).map_err(|_| Error::Configuration)?;
    if url.scheme() != "http"
        || url.path() != "/"
        || !url.port().is_some_and(|port| port >= 1024)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url
            .host_str()
            .and_then(|s| s.parse::<Ipv4Addr>().ok())
            .is_some_and(|ip| ip == Ipv4Addr::LOCALHOST)
    {
        return Err(Error::Configuration);
    }
    Ok(url.to_string())
}
pub struct IrodoriClient {
    client: Client,
    base: Url,
}
impl IrodoriClient {
    pub fn new(base: &str, timeout: Duration) -> Result<Self, Error> {
        let base = Url::parse(&canonical_endpoint(base)?).map_err(|_| Error::Configuration)?;
        if timeout.is_zero() || timeout > Duration::from_secs(1200) {
            return Err(Error::Configuration);
        }
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(timeout.min(Duration::from_secs(5)))
            .timeout(timeout)
            .build()
            .map_err(|_| Error::Configuration)?;
        Ok(Self { client, base })
    }
    /// Caller owns the engine and must settle it after a dispatched cancellation.
    pub async fn synthesize(
        &self,
        text: &str,
        settings: &Settings,
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>, Error> {
        cancelled(cancel)?;
        settings.validate().map_err(|_| Error::Configuration)?;
        if text.trim().is_empty() || text.contains('\0') || text.chars().count() > 4096 {
            return Err(Error::Configuration);
        }
        let body = serde_json::json!({"text": text, "settings": settings});
        let mut url = self.base.clone();
        url.set_path("v1/synthesize");
        let response = self
            .client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(&body).map_err(|_| Error::Configuration)?)
            .send()
            .await
            .map_err(failure)?;
        read_response(response, sungeki_media::MAX_WAV_BYTES, "audio/wav", cancel).await
    }
}
async fn read_response(
    mut response: reqwest::Response,
    limit: usize,
    content_type: &str,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, Error> {
    cancelled(cancel)?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(Error::Http(response.status().as_u16()));
    }
    if response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        != Some(content_type)
    {
        return Err(Error::InvalidResponse);
    }
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err(Error::TooLarge);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(failure)? {
        cancelled(cancel)?;
        if chunk.len() > limit - bytes.len() {
            return Err(Error::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    cancelled(cancel)?;
    Ok(bytes)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogWire {
    model: ModelCapabilities,
    voices: Vec<RegisteredVoice>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Health {
    pub status: String,
    pub busy: bool,
}
impl IrodoriClient {
    async fn get(&self, path: &str, cancel: &AtomicBool) -> Result<Vec<u8>, Error> {
        cancelled(cancel)?;
        let mut url = self.base.clone();
        url.set_path(path);
        let response = self.client.get(url).send().await.map_err(failure)?;
        read_response(response, 4 * 1024 * 1024, "application/json", cancel).await
    }
    pub async fn health(&self, cancel: &AtomicBool) -> Result<Health, Error> {
        let health: Health = serde_json::from_slice(&self.get("v1/health", cancel).await?)
            .map_err(|_| Error::InvalidResponse)?;
        if health.status != "ready" {
            return Err(Error::NotReady);
        }
        Ok(health)
    }
    pub async fn catalog(&self, cancel: &AtomicBool) -> Result<Catalog, Error> {
        let wire: CatalogWire = serde_json::from_slice(&self.get("v1/catalog", cancel).await?)
            .map_err(|_| Error::InvalidResponse)?;
        Catalog::new(wire.model, wire.voices).map_err(|_| Error::InvalidResponse)
    }
}
