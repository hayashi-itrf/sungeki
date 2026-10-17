//! Local Qwen3-TTS OpenAI-compatible adapter. No engine launch or Project edits.
//! Raw responses require media validation and model identity checks before use.
mod catalog;
mod provenance;
pub(crate) use provenance::reference_matches;
mod clip_input;
pub use clip_input::PreparedQwenClip;
mod exchange;
mod identity;
mod input;
mod job_input;
pub use catalog::{Catalog, Selection};
pub use exchange::{ExchangeFailure, ExchangeStage};
pub use input::PreparedQwenInput;
pub use job_input::QwenJobInput;
use reqwest::{Client, Url};
use std::{
    net::IpAddr,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
pub use sungeki_project::{QwenIdentity as RuntimeIdentity, QwenSettings as Settings};

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
        write!(f, "Qwen3-TTS: {self:?}")
    }
}
impl std::error::Error for Error {}

fn cancelled(flag: &AtomicBool) -> Result<(), Error> {
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
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url
            .host_str()
            .and_then(|s| s.trim_matches(['[', ']']).parse::<IpAddr>().ok())
            .is_some_and(|ip| ip.is_loopback())
    {
        return Err(Error::Configuration);
    }
    Ok(url.to_string())
}
pub struct QwenClient {
    client: Client,
    base: Url,
}
impl QwenClient {
    pub fn new(base: &str, timeout: Duration) -> Result<Self, Error> {
        let base = Url::parse(&canonical_endpoint(base)?).map_err(|_| Error::Configuration)?;
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
    /// Caller resolves pronunciation first. The server's extra normalization is
    /// disabled explicitly; no model/voice/speed/language defaults are inferred.
    /// Cancellation is checked at response/chunk boundaries, bounded by timeout.
    /// Dropping the request does not prove that remote inference has stopped.
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
        let mut body = serde_json::to_value(settings).map_err(|_| Error::Configuration)?;
        body["input"] = text.into();
        body["response_format"] = "wav".into();
        body["stream"] = false.into();
        body["normalization_options"] = serde_json::json!({"normalize":false});
        let mut url = self.base.clone();
        url.set_path("v1/audio/speech");
        let response = self
            .client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(&body).map_err(|_| Error::Configuration)?)
            .send()
            .await
            .map_err(failure)?;
        read_response(response, sungeki_media::MAX_WAV_BYTES, cancel).await
    }
}
async fn read_response(
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
