use crate::proxy::{PROXY_FORMAT_VERSION, ProxyKey};
use serde::{Deserialize, Serialize};
use sungeki_project::VideoProbe;

/// Untrusted cache metadata. A valid manifest does NOT establish frame decoding
/// or timing equivalence; media must revalidate these before editing playback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProxyManifest {
    pub version: u32,
    pub source: VideoProbe,
    pub derived: VideoProbe,
    pub conversion: String,
    pub encoder_sha256: String,
    pub source_pts: Vec<i64>,
    pub proxy_pts: Vec<i64>,
}
impl ProxyManifest {
    pub fn key(&self) -> Result<ProxyKey, &'static str> {
        if self.version != PROXY_FORMAT_VERSION
            || self.encoder_sha256.len() != 64
            || !self
                .encoder_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.conversion.is_empty()
            || self.conversion.len() > 4096
            || self.derived.byte_length > 1024 * 1024 * 1024
            || self.source_pts.is_empty()
            || self.source_pts.len() > 250_000
            || self.source_pts.len() != self.proxy_pts.len()
            || self.source_pts.windows(2).any(|p| p[0] >= p[1])
            || self.proxy_pts.windows(2).any(|p| p[0] >= p[1])
        {
            return Err("invalid proxy manifest");
        }
        self.source.validate().map_err(|_| "invalid source probe")?;
        self.derived
            .validate()
            .map_err(|_| "invalid derived probe")?;
        for (probe, pts) in [
            (&self.source, &self.source_pts),
            (&self.derived, &self.proxy_pts),
        ] {
            let stream = &probe.metadata.video;
            let (Some(origin), Some(duration)) = (stream.clock.start_pts, stream.clock.duration_ts)
            else {
                return Err("unknown proxy manifest timeline");
            };
            if stream
                .frame_count
                .is_some_and(|count| count != pts.len() as u64)
                || pts
                    .iter()
                    .any(|&pts| i128::from(pts) >= i128::from(origin) + i128::from(duration))
            {
                return Err("incomplete proxy manifest timeline");
            }
        }
        // JSON tuple is unambiguous even when the descriptor contains delimiters.
        let conversion = serde_json::to_vec(&(&self.conversion, &self.encoder_sha256))
            .map_err(|_| "invalid conversion descriptor")?;
        ProxyKey::new(&self.source, &conversion)
    }
}
