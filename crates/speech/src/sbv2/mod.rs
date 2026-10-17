//! Style-Bert-VITS2 adapter. Bytes remain unverified until media validation.
mod provenance;
pub(crate) use provenance::reference_matches;
mod exchange;
pub use exchange::{ExchangeFailure, ExchangeStage};
mod job_input;
pub use job_input::Sbv2JobInput;
mod clip_input;
pub use clip_input::PreparedSbv2Clip;
mod model_config;
pub use model_config::ModelConfig;
mod input;
pub use input::PreparedSbv2Input;
mod catalog;
pub use catalog::{Catalog, ModelInfo, Selection};
use reqwest::{Client, Url};
use std::{
    net::IpAddr,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
pub use sungeki_project::{Sbv2Language as Language, Sbv2Settings as Settings};

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Configuration,
    Cancelled,
    Connection,
    Timeout,
    Http(u16),
    TooLarge,
    InvalidResponse,
    ModelChanged,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Style-Bert-VITS2: {self:?}")
    }
}
impl std::error::Error for Error {}
fn transport(error: reqwest::Error) -> Error {
    // Request URLs contain speech text; never return URLs or response bodies.
    if error.is_timeout() {
        Error::Timeout
    } else {
        Error::Connection
    }
}
fn cancelled(flag: &AtomicBool) -> Result<(), Error> {
    if flag.load(Ordering::Acquire) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
/// Validate without connecting; persisted jobs share the transport endpoint policy.
pub(crate) fn endpoint(base: &str) -> Result<Url, Error> {
    let base = Url::parse(base).map_err(|_| Error::Configuration)?;
    let local = base
        .host_str()
        .and_then(|h| h.trim_matches(['[', ']']).parse::<IpAddr>().ok())
        .is_some_and(|ip| ip.is_loopback());
    if !local
        || base.scheme() != "http"
        || base.path() != "/"
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return Err(Error::Configuration);
    }
    Ok(base)
}
/// Normalize a configured loopback endpoint without opening a connection.
pub fn canonical_endpoint(base: &str) -> Result<String, Error> {
    Ok(endpoint(base)?.to_string())
}
pub struct Sbv2Client {
    client: Client,
    base: Url,
}
impl Sbv2Client {
    pub fn new(base: &str, timeout: Duration) -> Result<Self, Error> {
        let base = endpoint(base)?;
        if timeout.is_zero() || timeout > Duration::from_secs(300) {
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
    pub async fn models(&self, cancel: &AtomicBool) -> Result<serde_json::Value, Error> {
        cancelled(cancel)?;
        let mut url = self.base.clone();
        url.set_path("models/info");
        let response = self.client.get(url).send().await.map_err(transport)?;
        let bytes = read(response, 4 * 1024 * 1024, cancel).await?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| Error::InvalidResponse)?;
        if !value.is_object() {
            return Err(Error::InvalidResponse);
        }
        Ok(value)
    }
    pub async fn catalog(&self, cancel: &AtomicBool) -> Result<Catalog, Error> {
        Catalog::parse(self.models(cancel).await?)
    }
    /// Check the caller's selected server metadata before and after synthesis.
    /// File contents are not exposed by this API and still require separate identity checks.
    pub async fn synthesize_selected(
        &self,
        text: &str,
        settings: &Settings,
        expected: &Selection,
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>, Error> {
        if self.catalog(cancel).await?.select(settings)? != *expected {
            return Err(Error::ModelChanged);
        }
        let bytes = self.synthesize(text, settings, cancel).await?;
        if self.catalog(cancel).await?.select(settings)? != *expected {
            return Err(Error::ModelChanged);
        }
        Ok(bytes)
    }
    /// Returns unverified engine bytes. Validate WAV format, duration and hash before applying.
    /// Cancellation during an in-flight request is observed at response/chunk boundaries;
    /// the HTTP timeout bounds waits and does not prove remote synthesis has stopped.
    pub async fn synthesize(
        &self,
        text: &str,
        settings: &Settings,
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>, Error> {
        settings.validate().map_err(|_| Error::Configuration)?;
        cancelled(cancel)?;
        if text.trim().is_empty()
            || text.len() > crate::MAX_SPEECH_OUTPUT_BYTES
            || text.contains('\0')
        {
            return Err(Error::Configuration);
        }
        let mut url = self.base.clone();
        url.set_path("voice");
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("text", text);
            let fields = serde_json::to_value(settings).map_err(|_| Error::Configuration)?;
            for (key, value) in fields.as_object().ok_or(Error::Configuration)? {
                let encoded = match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                pairs.append_pair(key, &encoded);
            }
        }

        // The installed FastAPI server defines POST parameters as Query, not JSON.
        let response = self.client.post(url).send().await.map_err(transport)?;
        read(response, sungeki_media::MAX_WAV_BYTES, cancel).await
    }
}
async fn read(
    mut response: reqwest::Response,
    limit: usize,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, Error> {
    cancelled(cancel)?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(Error::Http(response.status().as_u16()));
    }
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err(Error::TooLarge);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport)? {
        cancelled(cancel)?;
        if chunk.len() > limit - bytes.len() {
            return Err(Error::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    cancelled(cancel)?;
    Ok(bytes)
}
