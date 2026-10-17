//! Stage-aware exchange. Dispatch ownership is the caller's responsibility.
use super::client::cancelled;
use super::{Error, IrodoriClient, PreparedIrodoriInput};
use std::sync::atomic::AtomicBool;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeStage {
    BeforeSynthesis,
    Synthesis,
    AfterSynthesis,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ExchangeFailure {
    pub stage: ExchangeStage,
    pub error: Error,
}
impl ExchangeFailure {
    /// A failed synthesis request can leave inference running. The owned engine
    /// must settle before its GPU/dispatch lease can be released.
    pub fn requires_engine_settlement(&self) -> bool {
        self.stage == ExchangeStage::Synthesis
    }
}
impl std::fmt::Display for ExchangeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Irodori {:?}: {}", self.stage, self.error)
    }
}
impl std::error::Error for ExchangeFailure {}
impl IrodoriClient {
    /// Caller must hold durable dispatch ownership and verify original model and
    /// voice files independently. This checks server metadata, not file identity.
    /// Returned bytes remain unverified until input.verify_audio succeeds.
    /// Dropping this future after dispatch also requires engine settlement.
    pub async fn exchange_prepared(
        &self,
        input: &PreparedIrodoriInput,
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>, ExchangeFailure> {
        self.check_selection(input, cancel)
            .await
            .map_err(|error| ExchangeFailure {
                stage: ExchangeStage::BeforeSynthesis,
                error,
            })?;
        let bytes = self
            .synthesize(
                &input.resolved().text,
                &input.audio_source().settings,
                cancel,
            )
            .await
            .map_err(|error| ExchangeFailure {
                stage: ExchangeStage::Synthesis,
                error,
            })?;
        self.check_selection(input, cancel)
            .await
            .map_err(|error| ExchangeFailure {
                stage: ExchangeStage::AfterSynthesis,
                error,
            })?;
        Ok(bytes)
    }
    async fn check_selection(
        &self,
        input: &PreparedIrodoriInput,
        cancel: &AtomicBool,
    ) -> Result<(), Error> {
        cancelled(cancel)?;
        if input.selection().is_none() {
            return Err(Error::Configuration);
        }
        let source = input.audio_source();
        let current = self
            .catalog(cancel)
            .await?
            .select(&source.settings, &source.identity)
            .map_err(|_| Error::ModelChanged)?;
        if current.audio_source() != source {
            return Err(Error::ModelChanged);
        }
        cancelled(cancel)
    }
}
