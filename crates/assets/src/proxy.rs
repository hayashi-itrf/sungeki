//! Content-addressed identity for derived previews. Never a Project asset locator.
use sha2::{Digest, Sha256};
use sungeki_project::VideoProbe;

/// Bump when generation or verification semantics change, even if settings do not.
pub const PROXY_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyKey(String);
impl ProxyKey {
    /// `conversion` must describe the complete conversion: encoder/version,
    /// dimensions, pixel format, display transform, audio policy and timing policy.
    /// The host must supply canonical bytes, independent of machine/file paths.
    pub fn new(source: &VideoProbe, conversion: &[u8]) -> Result<Self, &'static str> {
        Self::versioned(source, conversion, PROXY_FORMAT_VERSION)
    }
    fn versioned(
        source: &VideoProbe,
        conversion: &[u8],
        version: u32,
    ) -> Result<Self, &'static str> {
        source
            .validate()
            .map_err(|_| "invalid original video identity")?;
        if conversion.is_empty() || conversion.len() > 64 * 1024 || version == 0 {
            return Err("invalid proxy conversion descriptor");
        }
        let mut hash = Sha256::new();
        // Persistent hash domain: keep stable across product renames.
        hash.update(b"vvm-next-proxy\0");
        hash.update(version.to_be_bytes());
        // VideoProbe validation requires canonical lowercase SHA-256.
        hash.update(source.sha256.as_bytes());
        hash.update(source.byte_length.to_be_bytes());
        hash.update(source.metadata.video.index.to_be_bytes());
        hash.update((conversion.len() as u64).to_be_bytes());
        hash.update(conversion);
        Ok(Self(format!("{:x}", hash.finalize())))
    }
    /// Safe single path component; no absolute paths or traversal from descriptors.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An index candidate is NOT evidence of successful decoding or timing validation.
/// A cache hit must still verify the artifact bytes and its generation manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyCandidate {
    pub key: ProxyKey,
    artifact_sha256: String,
    artifact_bytes: u64,
}
impl ProxyCandidate {
    pub fn new(key: ProxyKey, sha256: &str, bytes: u64) -> Result<Self, &'static str> {
        if bytes == 0
            || bytes > 4 * 1024 * 1024 * 1024
            || sha256.len() != 64
            || !sha256.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("invalid proxy artifact identity");
        }
        Ok(Self {
            key,
            artifact_sha256: sha256.to_ascii_lowercase(),
            artifact_bytes: bytes,
        })
    }
    pub fn matches(&self, sha256: &str, bytes: u64) -> bool {
        self.artifact_bytes == bytes && self.artifact_sha256.eq_ignore_ascii_case(sha256)
    }
}

/// Bounded in-memory index. Eviction drops references only; it never deletes files.
/// Original assets/locators/Project history cannot be modified through this type.
pub struct ProxyIndex {
    entries: std::collections::VecDeque<ProxyCandidate>,
    capacity: usize,
}
impl ProxyIndex {
    pub fn new(capacity: usize) -> Result<Self, &'static str> {
        if !(1..=4096).contains(&capacity) {
            return Err("invalid proxy index capacity");
        }
        Ok(Self {
            entries: Default::default(),
            capacity,
        })
    }
    pub fn insert(&mut self, candidate: ProxyCandidate) {
        self.entries.retain(|entry| entry.key != candidate.key);
        self.entries.push_front(candidate);
        self.entries.truncate(self.capacity);
    }
    pub fn candidate(&mut self, key: &ProxyKey) -> Option<&ProxyCandidate> {
        let position = self.entries.iter().position(|entry| &entry.key == key)?;
        let entry = self.entries.remove(position)?;
        self.entries.push_front(entry);
        self.entries.front()
    }
    pub fn remove(&mut self, key: &ProxyKey) {
        self.entries.retain(|entry| &entry.key != key);
    }
}

#[cfg(test)]
#[path = "proxy_tests.rs"]
mod tests;
