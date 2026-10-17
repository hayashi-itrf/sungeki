mod locator;
pub use locator::{AssetLocationError, AssetRoots, rebase, resolve};
mod probe;
pub use probe::{ImageProbe, ProbeError, ProbeLimits, image_source_hash, probe_image};
mod preview;
pub use preview::image_preview;
mod snapshot;
pub use snapshot::{SnapshotError, snapshot_image};

#[cfg(unix)]
mod collection;
#[cfg(unix)]
mod collection_scan;
#[cfg(unix)]
pub use collection::{
    CollectionError, CollectionFile, CollectionInput, CollectionLimits, CollectionPlan,
    CollectionSource, CollectionSourceStatus, plan_collection,
};

pub mod proxy;

#[cfg(unix)]
pub mod proxy_store;

#[cfg(unix)]
pub mod proxy_cleanup;
