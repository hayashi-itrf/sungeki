//! Stage-aware exchange for durable host dispatch ownership.
use super::{Error, PreparedSbv2Input, Sbv2Client, cancelled};
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
    /// A failed synthesis request may still be executing remotely. Do not release
    /// its durable engine/GPU ownership until the owned process is settled.
    /// Preflight never sends synthesis; postflight follows a fully drained response.
    pub fn requires_engine_settlement(&self) -> bool {
        self.stage == ExchangeStage::Synthesis
    }
}
impl std::fmt::Display for ExchangeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SBV2 {:?}: {}", self.stage, self.error)
    }
}
impl std::error::Error for ExchangeFailure {}
impl Sbv2Client {
    /// Caller owns a durable dispatch lease and has rechecked original files.
    /// Success returns fully drained bytes, still requiring WAV verification.
    /// Dropping this future after dispatch also requires engine settlement.
    pub async fn exchange_prepared(
        &self,
        input: &PreparedSbv2Input,
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>, ExchangeFailure> {
        let preflight = async {
            cancelled(cancel)?;
            if input.identity().is_none() {
                return Err(Error::Configuration);
            }
            self.check_selection(input, cancel).await?;
            cancelled(cancel)
        }
        .await;
        preflight.map_err(|error| ExchangeFailure {
            stage: ExchangeStage::BeforeSynthesis,
            error,
        })?;
        let bytes = self
            .synthesize(&input.resolved().text, input.settings(), cancel)
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
        input: &PreparedSbv2Input,
        cancel: &AtomicBool,
    ) -> Result<(), Error> {
        if self.catalog(cancel).await?.select(input.settings())? != *input.selection() {
            return Err(Error::ModelChanged);
        }
        Ok(())
    }
}
