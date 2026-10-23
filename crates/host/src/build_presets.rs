//! Versioned bundled standing presets. Custom raster art has an explicit ID.
use crate::build_state::invalid;
use serde::Deserialize;
use sungeki_project::{BuildRecipe, BuildRole, BuildStanding, PsdSelection, Voice};
use sungeki_protocol::ApiError;

#[derive(Deserialize)]
struct Mouth {
    group: String,
    closed_option: String,
    open_option: String,
}
#[derive(Deserialize)]
struct Preset {
    preset_id: String,
    credit: String,
    source_sha256: String,
    selections: Vec<PsdSelection>,
    speech: Mouth,
    voice_preset_id: Option<String>,
}
#[derive(Deserialize)]
struct VoicePreset {
    preset_id: String,
    engine_id: String,
    style_id: u32,
    credit: String,
}
fn bundled(id: &str) -> Result<Preset, ApiError> {
    let text = match id {
        "zundamon-sakamoto-2.3-mouth" => {
            include_str!("../../../presets/characters/zundamon-sakamoto-2.3-mouth.json")
        }
        "zundamon-sakamoto-2.3-art-mouth" => {
            include_str!("../../../presets/characters/zundamon-sakamoto-2.3-art-mouth.json")
        }
        "metan-sakamoto-2.1-mouth" => {
            include_str!("../../../presets/characters/metan-sakamoto-2.1-mouth.json")
        }
        _ => return Err(invalid("未登録または制作未対応の立ち絵プリセットです")),
    };
    let preset: Preset = serde_json::from_str(text).map_err(invalid)?;
    if preset.preset_id != id {
        return Err(invalid("同梱プリセットIDが一致しません"));
    }
    Ok(preset)
}
pub(crate) fn validate(recipe: &BuildRecipe) -> Result<(), ApiError> {
    recipe.validate().map_err(invalid)?;
    for role in &recipe.roles {
        validate_role(recipe, role)?;
    }
    Ok(())
}
fn validate_role(recipe: &BuildRecipe, role: &BuildRole) -> Result<(), ApiError> {
    match &role.standing {
        BuildStanding::Vrm { .. } if role.preset_id == "vrm-custom-v1" => Ok(()),
        BuildStanding::Vrm { .. } => Err(invalid("VRM立ち絵はvrm-custom-v1を指定してください")),
        BuildStanding::Sprite { .. } if role.preset_id == "sprite-custom-v1" => Ok(()),
        BuildStanding::Sprite { .. } => {
            Err(invalid("差分PNG立ち絵はsprite-custom-v1を指定してください"))
        }
        BuildStanding::Image { .. } if role.preset_id == "image-custom-v1" => Ok(()),
        BuildStanding::Image { .. } => Err(invalid(
            "任意画像の立ち絵はimage-custom-v1を指定してください",
        )),
        // The shared recipe validator above still checks source hash, layers,
        // speech/character ownership and non-empty asset/voice credits.
        BuildStanding::Psd { .. } if role.preset_id == "psd-custom-v1" => Ok(()),
        BuildStanding::Psd {
            asset_id,
            pose,
            speech,
            ..
        } => {
            let preset = bundled(&role.preset_id)?;
            let asset = recipe
                .assets
                .iter()
                .find(|a| &a.id == asset_id)
                .ok_or_else(|| invalid("立ち絵素材がありません"))?;
            let mut selections = pose.selections.clone();
            let mut expected = preset.selections;
            selections.sort_by(|a, b| a.group.cmp(&b.group));
            expected.sort_by(|a, b| a.group.cmp(&b.group));
            if asset.probe.sha256() != preset.source_sha256
                || pose.source_sha256 != preset.source_sha256
                || selections != expected
                || speech.group != preset.speech.group
                || speech.closed_option != preset.speech.closed_option
                || speech.open_option != preset.speech.open_option
                || !asset.credit.lines().any(|s| s.trim() == preset.credit)
            {
                return Err(invalid(
                    "立ち絵原本・基本ポーズ・口パク・クレジットがプリセットと一致しません",
                ));
            }
            if let Some(id) = preset.voice_preset_id {
                if id != "zundamon-voice" {
                    return Err(invalid("未対応の音声プリセットです"));
                }
                let voice: VoicePreset = serde_json::from_str(include_str!(
                    "../../../presets/characters/zundamon-voice.json"
                ))
                .map_err(invalid)?;
                let Voice::Voicevox {
                    engine_id,
                    settings,
                    ..
                } = &role.character.voice
                else {
                    return Err(invalid("選択した音声プリセットはVOICEVOX専用です"));
                };
                if voice.preset_id != id
                    || *engine_id != voice.engine_id
                    || settings.style_id != voice.style_id
                    || !role
                        .character
                        .credit
                        .lines()
                        .any(|s| s.trim() == voice.credit)
                {
                    return Err(invalid("話者の音声・クレジットがプリセットと一致しません"));
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "build_presets_tests.rs"]
mod tests;
