//! Read-only profile discovery; never starts a model or claims actual synthesis.
use super::configuration;
use crate::sessions::Sessions;
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use sungeki_protocol::{
    ApiError, IrodoriProfileDetails, IrodoriProfileSummary, IrodoriSpeaker, SessionResponse,
};

pub(crate) fn list(shared: &Arc<Mutex<Sessions>>) -> Result<SessionResponse, ApiError> {
    let (database, flag) = crate::speech_api::profile_inspection(shared)?;
    let profiles = configuration::read_profiles(&database, &flag)?
        .into_iter()
        .map(|p| IrodoriProfileSummary {
            id: p.id,
            name: p.name,
        })
        .collect();
    Ok(SessionResponse::IrodoriProfiles { profiles })
}
pub(crate) fn get(shared: &Arc<Mutex<Sessions>>, id: &str) -> Result<SessionResponse, ApiError> {
    let (database, flag) = crate::speech_api::profile_inspection(shared)?;
    let profile = configuration::load(&database, id, &flag)?;
    Ok(SessionResponse::IrodoriProfile {
        details: inspect(&profile, &flag)?,
    })
}
pub(super) fn inspect(
    profile: &configuration::Profile,
    flag: &AtomicBool,
) -> Result<IrodoriProfileDetails, ApiError> {
    let catalog = profile.checked_catalog(flag)?;
    let details = IrodoriProfileDetails {
        profile_id: profile.id.clone(),
        name: profile.name.clone(),
        identity: profile.identity.clone(),
        speakers: catalog
            .voices()
            .map(|voice| IrodoriSpeaker {
                id: voice.id.clone(),
                name: voice.name.clone(),
            })
            .collect(),
        speaker_conditioning: catalog.model().speaker_conditioning,
        caption_conditioning: catalog.model().caption_conditioning,
        default_settings: profile.default_settings.clone(),
    };
    Ok(details)
}
