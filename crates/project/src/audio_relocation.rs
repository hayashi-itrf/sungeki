use crate::{AssetLocator, AudioProbe};
use serde::{Deserialize, Serialize};

/// Internal save intent data. Host must verify identical original bytes at the
/// destination before applying this relocation to any editor/checkpoint state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioRelocation {
    pub asset_id: String,
    pub source: AssetLocator,
    pub probe: AudioProbe,
    pub destination: AssetLocator,
}
