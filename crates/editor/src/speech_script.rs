use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::*;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeechScriptLine {
    pub character_id: Option<String>,
    pub clip_id: String,
    pub subtitle_text: String,
    pub spoken_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeechScript {
    pub lines: Vec<SpeechScriptLine>,
    pub character_id: String,
    pub track_id: String,
    pub scene_id: Option<String>,
    pub start_us: TimeUs,
    pub provisional_duration_us: TimeUs,
    pub gap_us: TimeUs,
}

pub(crate) fn append(p: &mut Project, script: &SpeechScript) -> Result<(), ValidationError> {
    let error = |s: &str| ValidationError::new(s.into());
    if script.lines.is_empty() || script.lines.len() > 200 {
        return Err(error("台本は1〜200行で指定してください"));
    }
    if !p.characters.iter().any(|c| c.id == script.character_id) {
        return Err(error("登録済みのキャラクターを指定してください"));
    }
    if !p
        .tracks
        .iter()
        .any(|t| t.id == script.track_id && t.kind == TrackKind::Audio)
    {
        return Err(error("音声トラックを指定してください"));
    }
    if script.provisional_duration_us.0 == 0 || script.gap_us.0 > MAX_TIME_US {
        return Err(error("仮の長さは正の値、間隔は0〜7日で指定してください"));
    }
    let mut cursor = script.start_us.0;
    let mut clips = Vec::new();
    for (index, line) in script.lines.iter().enumerate() {
        if line.subtitle_text.trim().is_empty() && line.spoken_text.is_none() {
            continue;
        }
        let character_id = line.character_id.as_ref().unwrap_or(&script.character_id);
        if !p.characters.iter().any(|c| c.id == *character_id) {
            return Err(ValidationError::new(format!(
                "台本の{}行目の話者が見つかりません",
                index + 1
            )));
        }
        let end = cursor
            .checked_add(script.provisional_duration_us.0)
            .filter(|end| *end <= MAX_TIME_US)
            .ok_or_else(|| {
                ValidationError::new(format!("台本の{}行目が7日の範囲を超えます", index + 1))
            })?;
        clips.push(Clip {
            id: line.clip_id.clone(),
            track_id: script.track_id.clone(),
            scene_id: script.scene_id.clone(),
            start_us: TimeUs(cursor),
            duration_us: script.provisional_duration_us,
            transform: Transform {
                motion: None,
                x: p.canvas.width as f64 * 0.05,
                y: p.canvas.height as f64 * 0.8,
                width: p.canvas.width as f64 * 0.9,
                height: p.canvas.height as f64 * 0.15,
                opacity: 1.0,
                fade: None,
                flip_x: None,
            },
            content: ClipContent::Speech {
                character_id: character_id.clone(),
                subtitle_text: line.subtitle_text.clone(),
                spoken_text: line.spoken_text.clone(),
                audio: None,
                gain_db: None,
            },
        });
        cursor = end
            .checked_add(script.gap_us.0)
            .ok_or_else(|| error("台本の配置時刻が大きすぎます"))?;
    }
    if clips.is_empty() {
        return Err(error("台本にセリフを入力してください"));
    }
    p.clips.extend(clips);
    Ok(())
}
