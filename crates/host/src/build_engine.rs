#[path = "build_profiles.rs"]
pub(crate) mod profiles;
use crate::build_state::invalid;
use serde::{Deserialize, Serialize};
use sungeki_project::{BuildRecipe, Voice};
use sungeki_protocol::ApiError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpeechConnection {
    pub endpoint: String,
    pub identity: sungeki_speech::VoicevoxIdentity,
}
impl SpeechConnection {
    pub(crate) fn canonical(mut self, recipe: &BuildRecipe) -> Result<Self, ApiError> {
        self.endpoint =
            sungeki_speech::voicevox::canonical_endpoint(&self.endpoint).map_err(invalid)?;
        self.validate(recipe)?;
        Ok(self)
    }
    pub(crate) fn validate(&self, recipe: &BuildRecipe) -> Result<(), ApiError> {
        if sungeki_speech::voicevox::canonical_endpoint(&self.endpoint).map_err(invalid)?
            != self.endpoint
        {
            return Err(invalid("合成先URLが正規化されていません"));
        }
        if [
            &self.identity.engine_id,
            &self.identity.engine_version,
            &self.identity.core_version,
        ]
        .iter()
        .any(|s| s.trim().is_empty() || s.len() > 128 || s.chars().any(char::is_control))
        {
            return Err(invalid("エンジン識別情報が不正です"));
        }
        for role in &recipe.roles {
            let Voice::Voicevox {
                engine_id,
                core_version,
                ..
            } = &role.character.voice
            else {
                continue;
            };
            if *engine_id != self.identity.engine_id || *core_version != self.identity.core_version
            {
                return Err(invalid("制作の話者と確認済み合成エンジンが一致しません"));
            }
        }
        Ok(())
    }
}
