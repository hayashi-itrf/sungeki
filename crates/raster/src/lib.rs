//! Native rasterization. No interpreter, shell, or external image worker.
mod composite;
pub mod psd;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
mod color_profile;
pub mod files;
pub mod frame;
pub mod preview;
#[cfg(test)]
mod tests;
mod text;
mod wrapping;
