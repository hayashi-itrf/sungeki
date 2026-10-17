//! Freeze managed profile configuration without duplicating large file inventories
//! into every parent record. Actual original hashes are checked by profile.prepare.
use crate::build_state::invalid;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};
use sungeki_project::{BuildRecipe, Voice};
use sungeki_protocol::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    Sbv2,
    Qwen,
    Irodori,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrozenProfile {
    pub kind: Kind,
    pub profile_id: String,
    pub configuration_sha256: String,
}
pub(crate) type Bindings = BTreeMap<String, FrozenProfile>;
pub(crate) enum LoadedProfile {
    Sbv2(crate::sbv2_originals::configuration::Profile),
    Qwen(crate::qwen_originals::configuration::Profile),
    Irodori(crate::irodori_originals::configuration::Profile),
}
impl LoadedProfile {
    pub fn load(
        database: &Path,
        kind: Kind,
        id: &str,
        cancel: &AtomicBool,
    ) -> Result<Self, ApiError> {
        Ok(match kind {
            Kind::Sbv2 => Self::Sbv2(crate::sbv2_originals::configuration::load(
                database, id, cancel,
            )?),
            Kind::Qwen => Self::Qwen(crate::qwen_originals::configuration::load(
                database, id, cancel,
            )?),
            Kind::Irodori => Self::Irodori(crate::irodori_originals::configuration::load(
                database, id, cancel,
            )?),
        })
    }
    pub fn freeze(&self) -> Result<FrozenProfile, ApiError> {
        match self {
            Self::Sbv2(p) => freeze(Kind::Sbv2, &p.id, p),
            Self::Qwen(p) => freeze(Kind::Qwen, &p.id, p),
            Self::Irodori(p) => freeze(Kind::Irodori, &p.id, p),
        }
    }
    /// Runs on the blocking preparation worker. This captures inputs only;
    /// engine ownership and parent reservation checks remain in the scheduler.
    pub fn prepare_child(
        &self,
        expected: &FrozenProfile,
        parent_id: &str,
        revision: u32,
        project: &sungeki_project::Project,
        clip_id: &str,
        cancel: &AtomicBool,
    ) -> Result<crate::speech_journal::Input, ApiError> {
        self.verify(expected)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire) {
            return Err(ApiError::new(
                sungeki_protocol::ErrorCode::Cancelled,
                "制作音声の準備を中止しました",
            ));
        }
        use crate::speech_journal::Input;
        // Each profile remeasures originals; the configuration digest alone
        // is never evidence that model files are unchanged.
        let input = match self {
            Self::Sbv2(p) => Input::Sbv2(
                p.prepare(revision, project, clip_id, cancel)?
                    .for_profile(&p.id)
                    .map_err(invalid)?
                    .for_parent(parent_id)
                    .map_err(invalid)?,
            ),
            Self::Qwen(p) => Input::Qwen(
                p.prepare(revision, project, clip_id, cancel)?
                    .for_profile(&p.id)
                    .map_err(invalid)?
                    .for_parent(parent_id)
                    .map_err(invalid)?,
            ),
            Self::Irodori(p) => Input::Irodori(
                p.prepare(revision, project, clip_id, cancel)?
                    .for_profile(&p.id)
                    .map_err(invalid)?
                    .for_parent(parent_id)
                    .map_err(invalid)?,
            ),
        };
        if cancel.load(std::sync::atomic::Ordering::Acquire) {
            return Err(ApiError::new(
                sungeki_protocol::ErrorCode::Cancelled,
                "制作音声の準備を中止しました",
            ));
        }
        Ok(input)
    }
    pub fn verify(&self, expected: &FrozenProfile) -> Result<(), ApiError> {
        if &self.freeze()? != expected {
            return Err(invalid("制作開始時の起動設定が変更されています"));
        }
        Ok(())
    }
}
fn freeze(kind: Kind, id: &str, profile: &impl Serialize) -> Result<FrozenProfile, ApiError> {
    // Value uses sorted object keys, independent of struct field declaration order.
    let value = serde_json::to_value(profile).map_err(invalid)?;
    let bytes = serde_json::to_vec(&("vvm-build-profile-v1", kind, id, value)).map_err(invalid)?;
    Ok(FrozenProfile {
        kind,
        profile_id: id.into(),
        configuration_sha256: format!("{:x}", Sha256::digest(bytes)),
    })
}
pub(crate) fn validate(bindings: &Bindings, recipe: &BuildRecipe) -> Result<(), ApiError> {
    let expected: BTreeMap<_, _> = recipe
        .roles
        .iter()
        .filter_map(|role| {
            let kind = match role.character.voice {
                Voice::Voicevox { .. } => return None,
                Voice::Sbv2 { .. } => Kind::Sbv2,
                Voice::Qwen { .. } => Kind::Qwen,
                Voice::Irodori { .. } => Kind::Irodori,
            };
            Some((role.character.id.as_str(), kind))
        })
        .collect();
    if bindings.len() != expected.len() {
        return Err(invalid(
            "制作話者の起動設定に不足または余分な指定があります",
        ));
    }
    for (id, profile) in bindings {
        if expected.get(id.as_str()) != Some(&profile.kind)
            || profile.profile_id.is_empty()
            || profile.profile_id.len() > 64
            || !profile
                .profile_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            || profile.configuration_sha256.len() != 64
            || !profile
                .configuration_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid("制作話者のエンジンまたは固定設定が不正です"));
        }
    }
    Ok(())
}
#[cfg(test)]
#[path = "build_profiles_tests.rs"]
pub(crate) mod tests;
