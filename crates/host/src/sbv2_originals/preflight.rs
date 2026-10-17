//! Offline candidate check. Does not acquire a host endpoint, open a database,
//! install configuration, or start any engine process.
use super::{configuration, discovery};
use std::{path::Path, sync::atomic::AtomicBool};
use sungeki_protocol::{ApiError, Sbv2ProfileDetails};

#[derive(serde::Serialize)]
pub(crate) struct Report {
    format_version: u32,
    check: &'static str,
    synthesis_checked: bool,
    dependency_completeness_checked: bool,
    profiles: Vec<Sbv2ProfileDetails>,
}
pub(crate) fn check(path: &Path, cancel: &AtomicBool) -> Result<Report, ApiError> {
    let profiles = configuration::read_candidate(path, cancel)?;
    let profiles = profiles
        .iter()
        .map(|profile| discovery::details(profile, cancel))
        .collect::<Result<_, _>>()?;
    Ok(Report {
        format_version: 1,
        check: "declared_originals",
        synthesis_checked: false,
        dependency_completeness_checked: false,
        profiles,
    })
}
