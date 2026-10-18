//! AivisSpeech and COEIROINK v2 local HTTP adapters.
mod input;
pub use input::PreparedLocalInput;
mod job;
pub use job::LocalSpeechJobInput;
mod client;
pub use client::LocalSpeechClient;
mod catalog;
pub use catalog::{LocalCatalog, LocalSpeaker, LocalStyle};
#[cfg(test)]
mod tests;
