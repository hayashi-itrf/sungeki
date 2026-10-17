use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use sungeki_project::*;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DialogueParticipant {
    pub character_id: String,
    pub standing_clip_id: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DialogueTemplate {
    pub preset_id: String,
    pub title: String,
    pub speech_clip_ids: Vec<String>,
    // Left then right. Each standing clip is an existing image or configured PSD.
    pub participants: Vec<DialogueParticipant>,
    pub background_clip_id: String,
    pub bgm_clip_id: String,
    pub start_us: TimeUs,
    pub gap_us: TimeUs,
    pub scene_id: String,
    // Backdrop, left, right, speech, music, overlay; new IDs supplied by caller.
    pub track_ids: Vec<String>,
    pub title_backdrop_id: String,
    pub credits_backdrop_id: String,
    pub title_clip_id: String,
    pub credits_clip_id: String,
}
fn error(text: &str) -> ValidationError {
    ValidationError::new(text.into())
}
fn clip<'a>(p: &'a Project, id: &str) -> Result<&'a Clip, ValidationError> {
    p.clips
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| error("テンプレートの対象クリップが見つかりません"))
}
pub(crate) fn apply(p: &mut Project, t: &DialogueTemplate) -> Result<(), ValidationError> {
    if t.preset_id != "dialogue-wide-v1"
        || t.participants.len() != 2
        || t.track_ids.len() != 6
        || p.canvas.width != 1920
        || p.canvas.height != 1080
        || p.canvas.fps.numerator != 30
        || p.canvas.fps.denominator != 1
    {
        return Err(error(
            "dialogue-wide-v1は1920×1080/30fps、2話者、6つの新規トラックIDが必要です",
        ));
    }
    if t.title.trim().is_empty() || t.title.len() > 4096 || t.title.contains('\0') {
        return Err(error("タイトルを4096バイト以内で指定してください"));
    }
    let mut selected = t.speech_clip_ids.clone();
    selected.extend([t.background_clip_id.clone(), t.bgm_clip_id.clone()]);
    selected.extend(t.participants.iter().map(|v| v.standing_clip_id.clone()));
    let unique: BTreeSet<_> = selected.iter().collect();
    if unique.len() != selected.len()
        || t.participants[0].character_id == t.participants[1].character_id
    {
        return Err(error("クリップと2人の話者は重複なしで指定してください"));
    }
    for id in &selected {
        if clip(p, id)?.scene_id.is_some() {
            return Err(error(
                "テンプレートにはシーン未所属のクリップを指定してください。既存の所属は自動解除しません",
            ));
        }
    }
    for participant in &t.participants {
        let character = p
            .characters
            .iter()
            .find(|c| c.id == participant.character_id)
            .ok_or_else(|| error("話者が見つかりません"))?;
        if character.credit.trim().is_empty() {
            return Err(error("話者のクレジットを入力してください"));
        }
        match &clip(p, &participant.standing_clip_id)?.content {
            ClipContent::Image { .. } => {}
            ClipContent::Psd { speech, .. }
                if speech
                    .as_ref()
                    .is_none_or(|s| s.character_id == participant.character_id) => {}
            _ => {
                return Err(error(
                    "立ち絵は画像か、対応する話者の口形を設定したPSDを指定してください",
                ));
            }
        }
    }
    let mut seen = BTreeSet::new();
    for id in &t.speech_clip_ids {
        match &clip(p, id)?.content {
            ClipContent::Speech { character_id, .. }
                if t.participants
                    .iter()
                    .any(|v| v.character_id == *character_id) =>
            {
                seen.insert(character_id.clone());
            }
            _ => return Err(error("発話は指定した2人のセリフを選んでください")),
        }
    }
    if seen.len() != 2 {
        return Err(error("両方の話者のセリフが必要です"));
    }
    if !matches!(
        clip(p, &t.background_clip_id)?.content,
        ClipContent::Image { .. }
    ) || !matches!(clip(p, &t.bgm_clip_id)?.content, ClipContent::Audio { .. })
    {
        return Err(error("背景画像と音声BGMを指定してください"));
    }
    let credits = crate::credits::text_for_clips(p, &selected)?;
    let speech_start = t
        .start_us
        .0
        .checked_add(3_000_000)
        .filter(|v| *v <= MAX_TIME_US)
        .ok_or_else(|| error("開始時刻が上限を超えます"))?;
    crate::speech_layout::layout(p, &t.speech_clip_ids, TimeUs(speech_start), t.gap_us)?;
    let speech_end = t
        .speech_clip_ids
        .iter()
        .map(|id| clip(p, id).map(|c| c.start_us.0 + c.duration_us.0))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max()
        .unwrap();
    let end = speech_end
        .checked_add(3_500_000)
        .filter(|v| *v <= MAX_TIME_US)
        .ok_or_else(|| error("終了時刻が上限を超えます"))?;
    if p.clips.iter().any(|c| {
        !unique.contains(&c.id)
            && c.start_us.0 < end
            && t.start_us.0 < c.start_us.0 + c.duration_us.0
    }) {
        return Err(error(
            "配置範囲に対象外のクリップがあります。空いた時間を選ぶか対象を整理してください",
        ));
    }
    p.scenes.push(Scene {
        fade: None,
        id: t.scene_id.clone(),
        name: t.title.clone(),
        start_us: t.start_us,
        duration_us: TimeUs(end - t.start_us.0),
    });
    for (i, name) in [
        "背景",
        "左の立ち絵",
        "右の立ち絵",
        "セリフ",
        "BGM",
        "タイトル・クレジット",
    ]
    .iter()
    .enumerate()
    {
        p.tracks.push(Track {
            id: t.track_ids[i].clone(),
            name: (*name).into(),
            kind: if i == 3 || i == 4 {
                TrackKind::Audio
            } else {
                TrackKind::Visual
            },
            visible: true,
            muted: false,
        });
    }
    for id in &selected {
        let index = p.clips.iter().position(|c| c.id == *id).unwrap();
        let track = if *id == t.background_clip_id {
            0
        } else if *id == t.bgm_clip_id {
            4
        } else if *id == t.participants[0].standing_clip_id {
            1
        } else if *id == t.participants[1].standing_clip_id {
            2
        } else {
            3
        };
        let geometry = if track == 0 || track == 1 || track == 2 {
            Some(super::dialogue_geometry::fit(p, &p.clips[index], track)?)
        } else {
            None
        };
        let c = &mut p.clips[index];
        c.scene_id = Some(t.scene_id.clone());
        c.track_id = t.track_ids[track].clone();
        if track != 3 {
            c.start_us = t.start_us;
            c.duration_us = TimeUs(
                if track == 0 || track == 4 {
                    end
                } else {
                    speech_end
                } - t.start_us.0,
            );
        }
        if let Some(geometry) = geometry {
            c.transform = geometry;
        }
        if track == 3 {
            c.transform = rect(360., 800., 1200., 240.);
        }
        if let ClipContent::Audio { playback, .. } = &mut c.content {
            playback.looped = true;
            playback.gain_db = -18.;
            playback.fade_in_us = TimeUs(1_000_000);
            playback.fade_out_us = TimeUs(1_000_000);
        }
    }
    for (id, start, duration) in [
        (&t.title_backdrop_id, t.start_us.0, 3_000_000),
        (&t.credits_backdrop_id, speech_end, 3_500_000),
    ] {
        p.clips.push(Clip {
            id: id.clone(),
            track_id: t.track_ids[5].clone(),
            scene_id: Some(t.scene_id.clone()),
            start_us: TimeUs(start),
            duration_us: TimeUs(duration),
            transform: rect(0., 0., 1920., 1080.),
            content: ClipContent::Shape {
                shape: ShapeKind::Rectangle,
                fill: "#ffffff".into(),
            },
        });
    }
    for (id, text, start, duration, transform, size) in [
        (
            &t.title_clip_id,
            t.title.clone(),
            t.start_us.0,
            3_000_000,
            rect(160., 150., 1600., 500.),
            64.,
        ),
        (
            &t.credits_clip_id,
            credits,
            speech_end,
            3_500_000,
            rect(160., 120., 1600., 880.),
            40.,
        ),
    ] {
        p.clips.push(Clip {
            id: id.clone(),
            track_id: t.track_ids[5].clone(),
            scene_id: Some(t.scene_id.clone()),
            start_us: TimeUs(start),
            duration_us: TimeUs(duration),
            transform,
            content: ClipContent::Text {
                outline: None,
                layout: None,
                text,
                font_size: size,
                color: "#111111".into(),
            },
        });
    }
    crate::track_separation::separate(p);
    Ok(())
}
pub(super) fn rect(x: f64, y: f64, width: f64, height: f64) -> Transform {
    Transform {
        motion: None,
        x,
        y,
        width,
        height,
        opacity: 1.,
        fade: None,
        flip_x: None,
    }
}
