use crate::{Project, Sbv2Identity, Sbv2Settings, ValidationError, VoicevoxSettings};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub id: String,
    pub name: String,
    pub credit: String,
    pub voice: Voice,
    pub subtitle_style: SubtitleStyle,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Voice {
    LocalHttp {
        settings: crate::LocalVoiceSettings,
    },
    Irodori {
        settings: crate::IrodoriSettings,
        identity: crate::IrodoriIdentity,
    },
    Qwen {
        settings: crate::QwenSettings,
        identity: crate::QwenIdentity,
    },
    Sbv2 {
        settings: Sbv2Settings,
        identity: Sbv2Identity,
    },
    Voicevox {
        engine_id: String,
        core_version: String,
        settings: VoicevoxSettings,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SubtitleOutline {
    pub width: u32,
    pub color: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SubtitleStyle {
    pub layout: Option<SubtitleLayout>,
    pub outline: Option<SubtitleOutline>,
    pub font_size: f64,
    pub color: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SubtitleLayout {
    pub alignment: TextAlignment,
    pub wrap: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}
pub(crate) fn validate_characters(project: &Project) -> Result<(), ValidationError> {
    for character in &project.characters {
        text(&character.name, 1024, false)?;
        text(&character.credit, 64 * 1024, true)?;
        match &character.voice {
            Voice::LocalHttp { settings } => settings.validate()?,
            Voice::Irodori { settings, identity } => {
                settings.validate()?;
                identity.validate()?;
            }
            Voice::Qwen { settings, identity } => {
                settings.validate()?;
                identity.validate()?;
            }
            Voice::Sbv2 { settings, identity } => {
                settings.validate()?;
                identity.validate()?;
            }
            Voice::Voicevox {
                engine_id,
                core_version,
                settings,
            } => {
                text(engine_id, 128, false)?;
                text(core_version, 128, false)?;
                if engine_id.chars().any(char::is_control)
                    || core_version.chars().any(char::is_control)
                {
                    return Err(ValidationError::new(
                        "エンジン識別情報に制御文字は使用できません".into(),
                    ));
                }
                settings.validate()?;
            }
        }
        if let Some(outline) = &character.subtitle_style.outline {
            if outline.width == 0 || outline.width > 32 || !crate::validate::color(&outline.color) {
                return Err(ValidationError::new(
                    "字幕の縁取りは1〜32pxと有効な色で指定してください".into(),
                ));
            }
        }
        if !character.subtitle_style.font_size.is_finite()
            || character.subtitle_style.font_size <= 0.0
            || !crate::validate::color(&character.subtitle_style.color)
        {
            return Err(ValidationError::new("字幕スタイルが不正です".into()));
        }
    }
    Ok(())
}
pub(crate) fn validate_speech(
    project: &Project,
    character_id: &str,
    subtitle: &str,
    spoken: Option<&str>,
) -> Result<(), ValidationError> {
    if !project.characters.iter().any(|c| c.id == character_id) {
        return Err(ValidationError::new(
            "音声クリップのキャラクターが見つかりません".into(),
        ));
    }
    text(subtitle, 64 * 1024, spoken.is_some())?;
    if let Some(spoken) = spoken {
        text(spoken, 64 * 1024, false)?;
    }
    Ok(())
}
fn text(value: &str, limit: usize, allow_empty: bool) -> Result<(), ValidationError> {
    if value.len() > limit || value.contains('\0') || (!allow_empty && value.trim().is_empty()) {
        return Err(ValidationError::new(format!(
            "文字列は{limit}バイト以内・NULなしで指定してください（必要な項目は空にできません）"
        )));
    }
    Ok(())
}
