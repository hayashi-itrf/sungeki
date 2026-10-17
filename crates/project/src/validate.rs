use crate::*;
use std::collections::{HashMap, HashSet};

fn require(ok: bool, message: &str) -> Result<(), ValidationError> {
    if ok {
        Ok(())
    } else {
        Err(ValidationError::new(message.into()))
    }
}
fn end(start: TimeUs, duration: TimeUs) -> Result<u64, ValidationError> {
    require(duration.0 > 0, "尺は正の値が必要です")?;
    start
        .0
        .checked_add(duration.0)
        .filter(|end| *end <= MAX_TIME_US)
        .ok_or_else(|| ValidationError::new("時刻が0〜7日の範囲外です".into()))
}
pub fn color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

pub fn validate(project: &Project) -> Result<(), ValidationError> {
    require(
        project.format == "vvm-project" && project.schema_version == 1,
        "未対応の保存形式です",
    )?;
    let ids = std::iter::once(&project.id)
        .chain(project.tracks.iter().map(|x| &x.id))
        .chain(project.scenes.iter().map(|x| &x.id))
        .chain(project.groups.iter().map(|x| &x.id))
        .chain(project.clips.iter().map(|x| &x.id))
        .chain(project.assets.iter().map(|x| &x.id))
        .chain(project.characters.iter().map(|x| &x.id));
    let mut unique = HashSet::new();
    for id in ids {
        let parsed = uuid::Uuid::parse_str(id)
            .map_err(|_| ValidationError::new("IDはUUIDが必要です".into()))?;
        require(
            id == &parsed.hyphenated().to_string() && unique.insert(parsed),
            "IDの形式が不正、または重複しています",
        )?;
    }
    let canvas = &project.canvas;
    require(
        canvas.width > 0
            && canvas.height > 0
            && canvas.fps.numerator > 0
            && canvas.fps.denominator > 0,
        "画面寸法・FPSは正の値が必要です",
    )?;
    require(
        color(&canvas.background),
        "背景色は#RRGGBBで指定してください",
    )?;
    if let Some(mix) = &project.audio_mix.normalization {
        require(
            mix.target_lufs.is_finite()
                && (-70.0..=0.0).contains(&mix.target_lufs)
                && mix.true_peak_db.is_finite()
                && (-20.0..=0.0).contains(&mix.true_peak_db),
            "音量調整値が範囲外です",
        )?;
    }
    crate::character::validate_characters(project)?;
    crate::asset_validation::validate_assets(&project.assets)?;
    validate_pronunciation_dictionary(&project.pronunciation_dictionary)?;
    let mut intervals = Vec::new();
    for scene in &project.scenes {
        if let Some(fade) = &scene.fade {
            fade.validate(scene.duration_us)?;
        }
        intervals.push((scene.start_us.0, end(scene.start_us, scene.duration_us)?));
    }
    intervals.sort_unstable();
    require(
        intervals.windows(2).all(|pair| pair[0].1 <= pair[1].0),
        "シーンの区間が重複しています",
    )?;
    let tracks: HashSet<_> = project.tracks.iter().map(|x| &x.id).collect();
    let scenes: HashMap<_, _> = project.scenes.iter().map(|x| (&x.id, x)).collect();
    for clip in &project.clips {
        let clip_end = end(clip.start_us, clip.duration_us)?;
        require(
            tracks.contains(&clip.track_id),
            "クリップのトラックが見つかりません",
        )?;
        require(
            project.tracks.iter().any(|t| {
                t.id == clip.track_id
                    && t.kind
                        == if matches!(
                            clip.content,
                            ClipContent::Speech { .. } | ClipContent::Audio { .. }
                        ) {
                            TrackKind::Audio
                        } else {
                            TrackKind::Visual
                        }
            }),
            "クリップの種類とトラックの種類が一致しません",
        )?;
        if let Some(id) = &clip.scene_id {
            let scene = scenes
                .get(id)
                .ok_or_else(|| ValidationError::new("シーンが見つかりません".into()))?;
            require(
                clip.start_us >= scene.start_us
                    && clip_end <= end(scene.start_us, scene.duration_us)?,
                "クリップが所属シーンの範囲外です",
            )?;
        }
        let t = &clip.transform;
        require(
            !t.flip_x.unwrap_or(false)
                || matches!(
                    clip.content,
                    ClipContent::Image { .. }
                        | ClipContent::Psd { .. }
                        | ClipContent::Video { .. }
                        | ClipContent::Sprite { .. }
                        | ClipContent::Vrm { .. }
                ),
            "左右反転は画像・PSD立ち絵・動画に設定してください",
        )?;
        if let Some(motion) = &t.motion {
            require(
                matches!(
                    clip.content,
                    ClipContent::Image { .. }
                        | ClipContent::Psd { .. }
                        | ClipContent::Video { .. }
                        | ClipContent::Sprite { .. }
                        | ClipContent::Vrm { .. }
                ),
                "移動・拡大は画像・PSD立ち絵・動画に設定してください",
            )?;
            motion.validate(t)?;
        }
        if let Some(fade) = &t.fade {
            require(
                !matches!(clip.content, ClipContent::Audio { .. }),
                "映像フェードは音声素材には設定できません",
            )?;
            fade.validate(clip.duration_us)?;
        }
        require(
            [t.x, t.y, t.width, t.height, t.opacity]
                .iter()
                .all(|v| v.is_finite())
                && t.width > 0.0
                && t.height > 0.0
                && (0.0..=1.0).contains(&t.opacity),
            "配置・寸法・透明度が不正です",
        )?;
        match &clip.content {
            ClipContent::Blur { sigma } => require(
                sigma.is_finite() && (0.0..=128.0).contains(sigma),
                "ぼかしの強さは0〜128で指定してください",
            )?,
            ClipContent::Video { asset_id, playback } => {
                let probe = project
                    .assets
                    .iter()
                    .find(|a| &a.id == asset_id)
                    .and_then(|a| match &a.probe {
                        AssetProbe::Video(p) => Some(p),
                        _ => None,
                    })
                    .ok_or_else(|| ValidationError::new("動画クリップの原本がありません".into()))?;
                playback.validate(probe, clip.duration_us)?;
            }
            ClipContent::Audio { asset_id, playback } => {
                let probe = project
                    .assets
                    .iter()
                    .find(|a| &a.id == asset_id)
                    .and_then(|a| {
                        if let AssetProbe::Audio(p) = &a.probe {
                            Some(p)
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| ValidationError::new("音声クリップの原本がありません".into()))?;
                playback.validate(probe, clip.duration_us)?;
            }
            ClipContent::Vrm { standing } => standing.validate(project, clip.duration_us)?,
            ClipContent::Sprite {
                sprites,
                pose,
                character_id,
            } => {
                sprites.validate(&project.assets)?;
                sprites.pose(pose.as_deref())?;
                require(
                    character_id
                        .as_ref()
                        .is_none_or(|id| project.characters.iter().any(|c| &c.id == id)),
                    "口パクのキャラクターがありません",
                )?;
            }
            ClipContent::Psd {
                asset_id,
                pose,
                speech,
            } => {
                let probe = project
                    .assets
                    .iter()
                    .find(|a| &a.id == asset_id)
                    .and_then(|a| {
                        if let AssetProbe::Psd(p) = &a.probe {
                            Some(p)
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| ValidationError::new("PSD原本の素材がありません".into()))?;
                pose.validate(probe)?;
                if let Some(binding) = speech {
                    require(
                        project
                            .characters
                            .iter()
                            .any(|c| c.id == binding.character_id),
                        "口パクのキャラクターがありません",
                    )?;
                    binding.validate(pose, probe)?;
                }
            }
            ClipContent::Speech {
                character_id,
                subtitle_text,
                spoken_text,
                audio,
                gain_db,
            } => {
                require(
                    gain_db.is_none_or(|db| {
                        db.is_finite()
                            && (crate::MIN_SPEECH_GAIN_DB..=crate::MAX_SPEECH_GAIN_DB).contains(&db)
                    }),
                    "発話の音量は-60〜24 dBで指定してください",
                )?;
                if let Some(audio) = audio {
                    audio.validate(project)?;
                }
                crate::character::validate_speech(
                    project,
                    character_id,
                    subtitle_text,
                    spoken_text.as_deref(),
                )?;
            }
            ClipContent::Image { asset_id } => require(
                project
                    .assets
                    .iter()
                    .any(|a| &a.id == asset_id && matches!(a.probe, AssetProbe::Image(_))),
                "画像クリップの素材が見つかりません",
            )?,
            ClipContent::Text {
                outline,
                font_size,
                color: fill,
                ..
            } => require(
                font_size.is_finite()
                    && *font_size > 0.0
                    && color(fill)
                    && outline
                        .as_ref()
                        .is_none_or(|o| (1..=32).contains(&o.width) && color(&o.color)),
                "文字スタイルが不正です",
            )?,
            ClipContent::Shape { fill, .. } => require(color(fill), "図形色が不正です")?,
        }
    }
    let clips: HashMap<_, _> = project.clips.iter().map(|x| (&x.id, x)).collect();
    let mut grouped = HashSet::new();
    for group in &project.groups {
        require(!group.clip_ids.is_empty(), "空のグループは作成できません")?;
        let mut membership = None;
        for id in &group.clip_ids {
            let clip = clips
                .get(id)
                .ok_or_else(|| ValidationError::new("グループのクリップが見つかりません".into()))?;
            require(grouped.insert(id), "クリップのグループ所属が重複しています")?;
            if let Some(scene) = membership {
                require(scene == &clip.scene_id, "グループがシーンをまたいでいます")?;
            }
            membership = Some(&clip.scene_id);
        }
    }
    Ok(())
}
