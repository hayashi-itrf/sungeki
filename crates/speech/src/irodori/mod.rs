//! Immutable Sungeki Irodori intent; no engine launch or Project mutation.
mod catalog;
mod input;
pub use catalog::{Catalog, ModelCapabilities, ReferenceKind, RegisteredVoice, SelectedVoice};
mod provenance;
pub use input::PreparedIrodoriInput;
pub(crate) use provenance::reference_matches;
mod clip_input;
mod job_input;
pub use clip_input::PreparedIrodoriClip;
pub use job_input::IrodoriJobInput;
mod client;
mod exchange;
pub use client::{Error, Health, IrodoriClient, canonical_endpoint};
pub use exchange::{ExchangeFailure, ExchangeStage};
