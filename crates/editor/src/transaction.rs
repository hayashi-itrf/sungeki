use crate::Command;
use sungeki_project::*;

pub fn apply_batch(project: &Project, commands: &[Command]) -> Result<Project, ValidationError> {
    let candidate = candidate(project, commands)?;
    crate::track_collision::validate_edit(project, &candidate)?;
    Ok(candidate)
}
pub(crate) fn candidate(
    project: &Project,
    commands: &[Command],
) -> Result<Project, ValidationError> {
    validate(project)?;
    let mut candidate = project.clone();
    for command in commands {
        apply(&mut candidate, command)?;
    }
    validate(&candidate)?;
    Ok(candidate)
}
fn missing() -> ValidationError {
    ValidationError::new("編集対象が見つかりません".into())
}
fn clip<'a>(p: &'a mut Project, id: &str) -> Result<&'a mut Clip, ValidationError> {
    p.clips.iter_mut().find(|c| c.id == id).ok_or_else(missing)
}
fn apply(p: &mut Project, command: &Command) -> Result<(), ValidationError> {
    match command {
        Command::Ripple {
            clip_ids,
            boundary_us,
            delta_us,
        } => crate::ripple::apply(p, clip_ids, *boundary_us, *delta_us)?,
        Command::ResizeLayout { width, height } => {
            let commands = crate::plan_resize_layout(p, *width, *height)?;
            *p = apply_batch(p, &commands)?;
        }
        Command::ApplyVoicePreset {
            preset_id,
            preset_revision,
            preset,
            character_id,
            credit,
        } => {
            if *preset_revision == 0 || uuid::Uuid::parse_str(preset_id).is_err() {
                return Err(ValidationError::new("プリセットのID・版が不正です".into()));
            }
            let commands = crate::plan_voice_preset(p, preset, character_id, credit)?;
            *p = apply_batch(p, &commands)?;
        }
        Command::ApplyCharacterPreset {
            preset_id,
            preset_revision,
            preset,
            ids,
            placement,
        } => {
            if *preset_revision == 0 || uuid::Uuid::parse_str(preset_id).is_err() {
                return Err(ValidationError::new("プリセットのID・版が不正です".into()));
            }
            let commands =
                crate::preset::plan_character_preset_candidate(p, preset, ids, placement.as_ref())?;
            *p = candidate(p, &commands)?;
        }
        Command::SetReactionMotion {
            clip_id,
            center_x,
            center_y,
            width,
            height,
            start_scale,
            fade,
        } => {
            crate::reaction::set(
                clip(p, clip_id)?,
                *center_x,
                *center_y,
                *width,
                *height,
                *start_scale,
                fade,
            )?;
        }
        Command::PrependForeword { template } => crate::foreword::prepend(p, template)?,
        Command::RefreshSceneCredits { clip_id, scene_id } => {
            crate::credits::refresh(p, clip_id, scene_id)?
        }
        Command::FitVisualToSpeech {
            clip_id,
            first_speech_id,
            last_speech_id,
        } => crate::visual_timing::fit(p, clip_id, first_speech_id, last_speech_id)?,
        Command::ApplyDialogueTemplate { template } => {
            crate::dialogue_template::apply(p, template)?
        }
        Command::LayoutSpeech {
            clip_ids,
            start_us,
            gap_us,
        } => crate::speech_layout::layout(p, clip_ids, *start_us, *gap_us)?,
        Command::AppendSpeechScript { script } => crate::speech_script::append(p, script)?,
        Command::SetAudioMix { audio_mix } => p.audio_mix = audio_mix.clone(),
        Command::SetAudio {
            clip_id,
            asset_id,
            playback,
        } => {
            let ClipContent::Audio {
                asset_id: current,
                playback: settings,
            } = &mut clip(p, clip_id)?.content
            else {
                return Err(ValidationError::new(
                    "音声素材クリップを指定してください".into(),
                ));
            };
            *current = asset_id.clone();
            *settings = playback.clone();
        }
        Command::SetPsdSpeech { clip_id, speech } => {
            let ClipContent::Psd {
                speech: current, ..
            } = &mut clip(p, clip_id)?.content
            else {
                return Err(ValidationError::new(
                    "口形を設定するPSDクリップを指定してください".into(),
                ));
            };
            *current = speech.clone();
        }
        Command::SetPsd {
            clip_id,
            asset_id,
            pose,
        } => match &mut clip(p, clip_id)?.content {
            ClipContent::Psd {
                asset_id: current,
                pose: selection,
                ..
            } => {
                *current = asset_id.clone();
                *selection = pose.clone();
            }
            _ => {
                return Err(ValidationError::new(
                    "PSD以外のクリップへポーズを設定できません".into(),
                ));
            }
        },
        Command::SetSpeechGain { .. }
        | Command::SetSpeechAudio { .. }
        | Command::SetCharacter { .. }
        | Command::RemoveCharacter { .. }
        | Command::SetSpeech { .. } => crate::speech_edit::apply(p, command)?,
        Command::SetPronunciationDictionary { entries } => {
            if p.pronunciation_dictionary != *entries {
                crate::speech_edit::clear_audio(p, |_| true);
                p.pronunciation_dictionary = entries.clone();
            }
        }
        Command::SetVrm { clip_id, standing } => {
            let content = &mut clip(p, clip_id)?.content;
            if !matches!(content, ClipContent::Vrm { .. }) {
                return Err(ValidationError::new("VRMクリップを指定してください".into()));
            }
            *content = ClipContent::Vrm {
                standing: standing.clone(),
            };
        }
        Command::SetSprite {
            clip_id,
            sprites,
            pose,
            character_id,
        } => {
            let content = &mut clip(p, clip_id)?.content;
            if !matches!(content, ClipContent::Sprite { .. }) {
                return Err(ValidationError::new(
                    "スプライトのクリップを指定してください".into(),
                ));
            }
            *content = ClipContent::Sprite {
                sprites: sprites.clone(),
                pose: pose.clone(),
                character_id: character_id.clone(),
            };
        }
        Command::SetVideo {
            clip_id,
            asset_id,
            playback,
        } => crate::video_edit::set(p, clip_id, asset_id, playback)?,
        Command::RegisterAsset { asset } => p.assets.push(asset.clone()),
        Command::SetAssetMetadata {
            asset_id,
            name,
            source,
            credit,
        } => {
            let asset = p
                .assets
                .iter_mut()
                .find(|a| a.id == *asset_id)
                .ok_or_else(missing)?;
            asset.name = name.clone();
            asset.source = source.clone();
            asset.credit = credit.clone();
        }
        Command::RelinkAsset {
            asset_id,
            locator,
            probe,
        } => crate::asset_relink::relink(p, asset_id, locator, probe)?,
        Command::RemoveAsset { asset_id } => {
            if !p.assets.iter().any(|a| a.id == *asset_id) {
                return Err(missing());
            }
            p.assets.retain(|a| a.id != *asset_id);
        }
        Command::SetImageAsset { clip_id, asset_id } => match &mut clip(p, clip_id)?.content {
            ClipContent::Image { asset_id: value } => *value = asset_id.clone(),
            _ => {
                return Err(ValidationError::new(
                    "画像以外のクリップへ素材を設定できません".into(),
                ));
            }
        },
        Command::ReplaceProject { project, .. } => {
            if project.id != p.id {
                return Err(ValidationError::new("別の作品には置換できません".into()));
            }
            validate(project)?;
            *p = project.clone();
        }
        Command::SetCanvas { canvas } => p.canvas = canvas.clone(),
        Command::RenameProject { title } => p.title = title.clone(),
        Command::AddTrack { track } => p.tracks.push(track.clone()),
        Command::SetTrackProperties {
            track_id,
            name,
            visible,
            muted,
        } => {
            crate::track_edit::properties(p, track_id, name, *visible, *muted)?;
        }
        Command::ReorderTracks { track_ids } => crate::track_edit::reorder(p, track_ids)?,
        Command::SetClipTrack { clip_id, track_id } => {
            clip(p, clip_id)?.track_id = track_id.clone()
        }
        Command::AddClip { clip } => p.clips.push(clip.clone()),
        Command::DuplicateClip {
            clip_id,
            new_clip_id,
            start_us,
        } => {
            crate::duplicate::clip(p, clip_id, new_clip_id, *start_us)?;
        }
        Command::SetText { clip_id, text } => match &mut clip(p, clip_id)?.content {
            ClipContent::Text { text: value, .. } => *value = text.clone(),
            _ => {
                return Err(ValidationError::new(
                    "文字以外のクリップへ文章を設定できません".into(),
                ));
            }
        },
        Command::SetTextOutline { clip_id, outline } => {
            crate::visual_content::text_outline(&mut clip(p, clip_id)?.content, outline)?;
        }
        Command::SetTextLayout { clip_id, layout } => {
            crate::visual_content::text_layout(&mut clip(p, clip_id)?.content, layout)?;
        }
        Command::SetTextStyle {
            clip_id,
            font_size,
            color,
        } => {
            crate::visual_content::text_style(&mut clip(p, clip_id)?.content, *font_size, color)?;
        }
        Command::SetBlur { clip_id, sigma } => {
            crate::visual_content::blur(&mut clip(p, clip_id)?.content, *sigma)?;
        }
        Command::SetShape {
            clip_id,
            shape,
            fill,
        } => {
            crate::visual_content::shape(&mut clip(p, clip_id)?.content, shape, fill)?;
        }
        Command::SetTransform { clip_id, transform } => {
            clip(p, clip_id)?.transform = transform.clone()
        }
        Command::MoveClip { clip_id, start_us } => clip(p, clip_id)?.start_us = *start_us,
        Command::ResizeClip {
            clip_id,
            duration_us,
        } => clip(p, clip_id)?.duration_us = *duration_us,
        Command::SetClipScene { clip_id, scene_id } => {
            clip(p, clip_id)?.scene_id = scene_id.clone()
        }
        Command::DeleteClip { clip_id } => {
            clip(p, clip_id)?;
            p.clips.retain(|c| c.id != *clip_id);
            for group in &mut p.groups {
                group.clip_ids.retain(|id| id != clip_id);
            }
            p.groups.retain(|g| !g.clip_ids.is_empty());
        }
        Command::AddScene { scene } => p.scenes.push(scene.clone()),
        Command::MoveScene { scene_id, start_us } => {
            crate::scene_edit::move_scene(p, scene_id, *start_us)?
        }
        Command::SetSceneFade { scene_id, fade } => {
            p.scenes
                .iter_mut()
                .find(|s| &s.id == scene_id)
                .ok_or_else(|| ValidationError::new("シーンが見つかりません".into()))?
                .fade = fade.clone();
        }
        Command::RenameScene { scene_id, name } => {
            crate::scene_edit::scene(p, scene_id)?.name = name.clone()
        }
        Command::ResizeScene {
            scene_id,
            duration_us,
        } => crate::scene_edit::scene(p, scene_id)?.duration_us = *duration_us,
        Command::DeleteScene { scene_id } => crate::scene_edit::remove_scene(p, scene_id, true)?,
        Command::UngroupScene { scene_id } => crate::scene_edit::remove_scene(p, scene_id, false)?,
        Command::LayoutScenes {
            scene_ids,
            start_us,
            gap_us,
        } => crate::scene_edit::layout(p, scene_ids, *start_us, *gap_us)?,
        Command::RenameGroup { group_id, name } => {
            p.groups
                .iter_mut()
                .find(|g| g.id == *group_id)
                .ok_or_else(missing)?
                .name = name.clone();
        }
        Command::MoveGroup { group_id, start_us } => {
            let ids = crate::scene_edit::group_clips(p, group_id)?;
            let start = crate::scene_edit::first_start(p, &ids)?;
            crate::scene_edit::move_selection(
                p,
                &[],
                std::slice::from_ref(group_id),
                &[],
                crate::scene_edit::delta(*start_us, start)?,
            )?;
        }
        Command::MoveSelection {
            scene_ids,
            group_ids,
            clip_ids,
            delta_us,
        } => crate::scene_edit::move_selection(p, scene_ids, group_ids, clip_ids, *delta_us)?,
        Command::DuplicateScene {
            scene_id,
            new_scene_id,
            start_us,
            clip_ids,
            group_ids,
        } => crate::duplicate::scene(p, scene_id, new_scene_id, *start_us, clip_ids, group_ids)?,
        Command::DuplicateGroup {
            group_id,
            new_group_id,
            start_us,
            clip_ids,
        } => crate::duplicate::group(p, group_id, new_group_id, *start_us, clip_ids)?,
        Command::AddGroup { group } => p.groups.push(group.clone()),
        Command::Ungroup { group_id } => {
            if !p.groups.iter().any(|g| g.id == *group_id) {
                return Err(missing());
            }
            p.groups.retain(|g| g.id != *group_id);
        }
    }
    Ok(())
}
