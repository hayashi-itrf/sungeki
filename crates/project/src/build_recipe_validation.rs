use crate::*;
use std::collections::HashSet;
fn require(ok: bool, message: &str) -> Result<(), ValidationError> {
    if ok {
        Ok(())
    } else {
        Err(ValidationError::new(message.into()))
    }
}
fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}
impl BuildRecipe {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(presentation) = &self.presentation {
            presentation.validate(self)?;
        }
        let role_count = match self.preset_id.as_str() {
            "dialogue-wide-v1" => 2,
            "narration-wide-v1" | "narration-portrait-v1" => 1,
            _ => return Err(ValidationError::new("未対応の制作プリセットです".into())),
        };
        require(text(&self.title, 1024), "作品名が不正です")?;
        require(
            self.roles.len() == role_count,
            "制作プリセットと話者数が一致しません",
        )?;
        require(
            !self.assets.is_empty() && self.assets.len() <= 512,
            "素材数は1〜512です",
        )?;
        require(
            !self.scenes.is_empty() && self.scenes.len() <= 200,
            "シーン数は1〜200です",
        )?;
        let count: usize = self.scenes.iter().map(|s| s.lines.len()).sum();
        require((1..=200).contains(&count), "全体の発話数は1〜200です")?;
        // Reuse Project's canonical IDs, probes, voice/style and dictionary rules.
        // This temporary validation value is never persisted or edited.
        let ids: HashSet<_> = self
            .assets
            .iter()
            .map(|a| a.id.as_str())
            .chain(self.roles.iter().map(|r| r.character.id.as_str()))
            .collect();
        let temporary_id = (0..)
            .map(|n| uuid::Uuid::from_u128(n).to_string())
            .find(|id| !ids.contains(id.as_str()))
            .unwrap();
        let mut p = Project::empty(temporary_id, self.title.clone());
        p.assets = self.assets.clone();
        p.characters = self.roles.iter().map(|r| r.character.clone()).collect();
        p.audio_mix = self.audio_mix.clone();
        p.pronunciation_dictionary = self.pronunciation_dictionary.clone();
        validate(&p)?;
        let mut roles = HashSet::new();
        for role in &self.roles {
            require(
                text(&role.role, 128) && roles.insert(&role.role) && text(&role.preset_id, 128),
                "話者の役名・プリセットIDが不正または重複しています",
            )?;
            require(
                text(&role.character.credit, 65536),
                "話者のクレジットが必要です",
            )?;
            role.standing.validate_character(&p, &role.character.id)?;
        }
        for (index, scene) in self.scenes.iter().enumerate() {
            self.scene(scene, &p)
                .map_err(|e| ValidationError::new(format!("シーン{}: {}", index + 1, e.0)))?;
        }
        if let Some(bgm) = &self.bgm {
            let AssetProbe::Audio(probe) = &self.asset(&bgm.asset_id)?.probe else {
                return Err(ValidationError::new("BGMは音声素材が必要です".into()));
            };
            require(
                bgm.playback.looped,
                "制作BGMは実尺まで繰り返す設定が必要です",
            )?;
            // Exact fades versus final duration are checked after layout.
            bgm.playback.validate(probe, TimeUs(MAX_TIME_US))?;
            if let Some(duration) = bgm.volume_point_reference_duration_us {
                require(
                    duration.0 > 0 && duration.0 <= MAX_TIME_US,
                    "BGM音量ポイントの基準時間が不正です",
                )?;
                crate::volume_point::validate_points(&bgm.playback.volume_points, duration)?;
            }
        }
        if let Some(f) = &self.foreword {
            require(
                f.preset_id
                    == if self.preset_id == "narration-portrait-v1" {
                        "foreword-portrait-v1"
                    } else {
                        "foreword-wide-v1"
                    }
                    && text(&f.text, 4096)
                    && f.text.lines().count() <= 4
                    && (1_600_000..=60_000_000).contains(&f.duration_us.0),
                "前書きの指定が不正です",
            )?;
        }
        Ok(())
    }
    fn asset(&self, id: &str) -> Result<&Asset, ValidationError> {
        let asset = self
            .assets
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| ValidationError::new("参照素材がありません".into()))?;
        require(text(&asset.credit, 65536), "使用素材のクレジットが必要です")?;
        Ok(asset)
    }
    fn image(&self, id: &str) -> Result<(), ValidationError> {
        require(
            matches!(self.asset(id)?.probe, AssetProbe::Image(_)),
            "画像素材が必要です",
        )
    }
    fn scene(&self, s: &BuildScene, p: &Project) -> Result<(), ValidationError> {
        require(
            text(&s.name, 1024) && !s.lines.is_empty(),
            "シーン名と発話が必要です",
        )?;
        require(s.gap_us.0 <= 60_000_000, "発話間隔は60秒以内です")?;
        if let Some(fade) = &s.fade {
            fade.validate(TimeUs(MAX_TIME_US))?;
        }
        self.image(&s.background_asset_id)?;
        for (i, line) in s.lines.iter().enumerate() {
            self.validate_psd_states(line, p)
                .map_err(|e| ValidationError::new(format!("{}行目: {}", i + 1, e.0)))?;
            let role = self
                .roles
                .iter()
                .find(|r| r.role == line.role)
                .ok_or_else(|| ValidationError::new(format!("{}行目: 未登録の役名です", i + 1)))?;
            crate::character::validate_speech(
                p,
                &role.character.id,
                &line.subtitle_text,
                line.spoken_text.as_deref(),
            )
            .map_err(|e| ValidationError::new(format!("{}行目: {}", i + 1, e.0)))?;
            if let Some(name) = &line.sprite_pose {
                let BuildStanding::Sprite { sprites, .. } = &role.standing else {
                    return Err(ValidationError::new(format!(
                        "{}行目: sprite以外へsprite_poseは指定できません",
                        i + 1
                    )));
                };
                sprites.pose(Some(name))?;
            }
            if let Some(pose) = &line.pose {
                let BuildStanding::Psd {
                    asset_id, speech, ..
                } = &role.standing
                else {
                    return Err(ValidationError::new(format!(
                        "{}行目: PSD以外へposeは指定できません",
                        i + 1
                    )));
                };
                let AssetProbe::Psd(probe) = &self.asset(asset_id)?.probe else {
                    unreachable!()
                };
                pose.validate(probe)?;
                speech.validate(pose, probe)?;
            }
        }
        require(s.diagrams.len() <= 200, "説明図はシーンあたり200個以内です")?;
        let mut occupied = HashSet::new();
        for d in &s.diagrams {
            if let Some(fade) = &d.fade {
                require(
                    d.reaction.is_none(),
                    "通常フェードとリアクションは併用できません",
                )?;
                fade.validate(TimeUs(MAX_TIME_US))?;
            }
            if let Some(motion) = &d.motion {
                require(
                    d.reaction.is_none() && d.video_playback.is_none(),
                    "説明図の移動は通常画像だけに指定できます",
                )?;
                motion.validate_parameters()?;
            }
            if let Some(reaction) = &d.reaction {
                require(
                    d.video_playback.is_none(),
                    "リアクションには画像素材が必要です",
                )?;
                reaction.validate()?;
            }
            if let Some(playback) = &d.video_playback {
                let AssetProbe::Video(probe) = &self.asset(&d.asset_id)?.probe else {
                    return Err(ValidationError::new(
                        "動画再生設定には動画素材が必要です".into(),
                    ));
                };
                // Actual line duration is available only after speech layout.
                playback.validate(probe, TimeUs(1))?;
                VideoDisplayGeometry::new(&probe.metadata.video)
                    .map_err(|e| ValidationError::new(e.into()))?;
            } else {
                self.image(&d.asset_id)?;
            }
            require(
                d.first_line <= d.last_line && usize::from(d.last_line) < s.lines.len(),
                "説明図の発話範囲が不正です",
            )?;
            if d.reaction.is_none() {
                for i in d.first_line..=d.last_line {
                    require(occupied.insert(i), "説明図の発話範囲が重複しています")?;
                }
            }
        }
        Ok(())
    }
}
