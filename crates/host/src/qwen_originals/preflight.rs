//! Offline checks only: no host endpoint, database, installation or model startup.
use super::{configuration, profiles};
use std::{path::Path, sync::atomic::AtomicBool};
use sungeki_protocol::{ApiError, QwenProfileDetails};
#[derive(serde::Serialize)]
pub(crate) struct Report {
    format_version: u32,
    check: &'static str,
    synthesis_checked: bool,
    dependency_completeness_checked: bool,
    profiles: Vec<QwenProfileDetails>,
}
pub(crate) fn check(path: &Path, cancel: &AtomicBool) -> Result<Report, ApiError> {
    let profiles = configuration::read_candidate(path, cancel)?
        .iter()
        .map(|profile| profiles::inspect(profile, cancel))
        .collect::<Result<_, _>>()?;
    Ok(Report {
        format_version: 1,
        check: "declared_originals",
        synthesis_checked: false,
        dependency_completeness_checked: false,
        profiles,
    })
}
