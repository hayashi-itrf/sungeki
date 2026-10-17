//! Read-only profile discovery; never starts a model or claims actual synthesis.
use super::configuration;
use crate::sessions::Sessions;
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use sungeki_protocol::{
    ApiError, QwenProfileDetails, QwenProfileSummary, QwenSpeaker, SessionResponse,
};

pub(crate) fn list(shared: &Arc<Mutex<Sessions>>) -> Result<SessionResponse, ApiError> {
    let (database, flag) = crate::speech_api::profile_inspection(shared)?;
    let profiles = configuration::read_profiles(&database, &flag)?
        .into_iter()
        .map(|p| QwenProfileSummary {
            id: p.id,
            name: p.name,
        })
        .collect();
    Ok(SessionResponse::QwenProfiles { profiles })
}
pub(crate) fn get(shared: &Arc<Mutex<Sessions>>, id: &str) -> Result<SessionResponse, ApiError> {
    let (database, flag) = crate::speech_api::profile_inspection(shared)?;
    let profile = configuration::load(&database, id, &flag)?;
    Ok(SessionResponse::QwenProfile {
        details: inspect(&profile, &flag)?,
    })
}
pub(super) fn inspect(
    profile: &configuration::Profile,
    flag: &AtomicBool,
) -> Result<QwenProfileDetails, ApiError> {
    profile
        .binding
        .checked_command(profile.port, &profile.identity, flag)?;
    let catalog = profile.catalog()?;
    let details = QwenProfileDetails {
        profile_id: profile.id.clone(),
        name: profile.name.clone(),
        identity: profile.identity.clone(),
        speakers: catalog
            .voices()
            .map(|(id, name)| QwenSpeaker {
                id: id.into(),
                name: name.into(),
            })
            .collect(),
        languages: catalog.languages().map(str::to_owned).collect(),
        default_settings: profile.default_settings.clone(),
    };
    Ok(details)
}
