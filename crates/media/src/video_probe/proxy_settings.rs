use super::ProbeError;
use serde::{Deserialize, Serialize};

/// Stored in each generation job; never infer an old job's codec from defaults.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyEncoding {
    #[default]
    Png,
    Qtrle,
}
impl ProxyEncoding {
    pub fn codec(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Qtrle => "qtrle",
        }
    }
    pub fn pixels(self) -> &'static str {
        match self {
            Self::Png => "rgba",
            Self::Qtrle => "argb",
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct ProxySettings {
    pub max_edge: u32,
    pub encoding: ProxyEncoding,
}
impl ProxySettings {
    pub fn new(max_edge: u32) -> Self {
        Self {
            max_edge,
            encoding: ProxyEncoding::Qtrle,
        }
    }
    pub fn descriptor(&self) -> Result<String, ProbeError> {
        if !(64..=2048).contains(&self.max_edge) {
            return Err(ProbeError::Invalid("proxy maximum edge"));
        }
        let format = match self.encoding {
            ProxyEncoding::Png => "v1;mov-png-rgba",
            ProxyEncoding::Qtrle => "v2;mov-qtrle-argb;gop=30",
        };
        Ok(format!(
            "{format};bilinear;edge={};display=preserve;audio=original;pts=exact",
            self.max_edge
        ))
    }
}
