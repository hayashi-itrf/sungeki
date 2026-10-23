//! Compose a fixed opening through the same transaction as synthesized scenes.
use crate::{BuildLayoutIds, Command, dialogue_template::rect};
use serde::{Deserialize, Serialize};
use sungeki_project::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildOpeningIds {
    pub logo: Option<String>,
    pub audio: Vec<String>,
    pub performers: Vec<Vec<String>>,
}
impl BuildOpeningIds {
    pub fn matches(&self, opening: &BuildOpening) -> bool {
        self.logo.is_some() == opening.logo.is_some()
            && self.audio.len() == opening.audio.len()
            && self.performers.len() == opening.performers.len()
            && self
                .performers
                .iter()
                .zip(&opening.performers)
                .all(|(ids, p)| ids.len() == p.intervals(opening.duration_us).len())
    }
}
pub(super) fn plan(
    p: &Project,
    recipe: &BuildRecipe,
    opening: &BuildOpening,
    ids: &BuildLayoutIds,
    extra: &BuildOpeningIds,
) -> Result<Vec<Command>, ValidationError> {
    opening.validate(recipe)?;
    if !extra.matches(opening) {
        return Err(ValidationError::new(
            "冒頭構成の固定ID数が一致しません".into(),
        ));
    }
    let mut commands = vec![Command::AddScene {
        scene: Scene {
            id: ids.title_scene.clone(),
            name: "タイトルコール・テーマ".into(),
            start_us: TimeUs(0),
            duration_us: opening.duration_us,
            fade: None,
        },
    }];
    let mut add = |id: &str, track: &str, range: &BuildOpeningRange, content, transform| {
        commands.push(Command::AddClip {
            clip: Clip {
                id: id.into(),
                track_id: track.into(),
                scene_id: Some(ids.title_scene.clone()),
                start_us: range.start_us,
                duration_us: range.duration_us,
                content,
                transform,
            },
        });
    };
    let full = BuildOpeningRange {
        start_us: TimeUs(0),
        duration_us: opening.duration_us,
    };
    let canvas = rect(0., 0., p.canvas.width as f64, p.canvas.height as f64);
    add(
        &ids.title_clips[0],
        &ids.tracks[0],
        &full,
        ClipContent::Image {
            asset_id: opening.background_asset_id.clone(),
        },
        canvas.clone(),
    );
    if let (Some(logo), Some(id)) = (&opening.logo, &extra.logo) {
        let mut transform = fit_visual_transform(
            &p.assets,
            &ClipContent::Image {
                asset_id: logo.asset_id.clone(),
            },
            &logo.transform,
            recipe
                .presentation
                .as_ref()
                .and_then(|p| p.fitting.as_ref())
                .and_then(|f| f.opening_logo.as_ref()),
        )?;
        transform.fade = logo.fade.clone();
        add(
            id,
            &ids.tracks[0],
            &logo.range,
            ClipContent::Image {
                asset_id: logo.asset_id.clone(),
            },
            transform,
        );
    }
    for (audio, id) in opening.audio.iter().zip(&extra.audio) {
        add(
            id,
            &ids.tracks[4],
            &audio.range,
            ClipContent::Audio {
                asset_id: audio.asset_id.clone(),
                playback: audio.playback.clone(),
            },
            canvas.clone(),
        );
    }
    for (performer, fixed) in opening.performers.iter().zip(&extra.performers) {
        let (index, role) = recipe
            .roles
            .iter()
            .enumerate()
            .find(|(_, r)| r.role == performer.role)
            .ok_or_else(|| ValidationError::new("冒頭の出演者が未登録です".into()))?;
        let BuildStanding::Psd {
            asset_id,
            pose,
            speech,
            ..
        } = &role.standing
        else {
            return Err(ValidationError::new("冒頭の口パクはPSDが必要です".into()));
        };
        for ((range, open), id) in performer.intervals(opening.duration_us).iter().zip(fixed) {
            add(
                id,
                &ids.tracks[index + 1],
                range,
                ClipContent::Psd {
                    asset_id: asset_id.clone(),
                    pose: speech.pose(pose, *open),
                    // These are authored mouth cues; do not bind them to synthesized speech.
                    speech: None,
                },
                fit_visual_transform(
                    &p.assets,
                    &ClipContent::Psd {
                        asset_id: asset_id.clone(),
                        pose: pose.clone(),
                        speech: None,
                    },
                    &performer.transform,
                    recipe
                        .presentation
                        .as_ref()
                        .and_then(|p| p.fitting.as_ref())
                        .and_then(|f| f.opening_standing.get(&performer.role)),
                )?,
            );
        }
    }
    let theme = &opening.theme;
    add(
        &ids.title_clips[1],
        &ids.tracks[5],
        &theme.range,
        ClipContent::Text {
            text: theme.text.clone(),
            font_size: theme.font_size,
            color: theme.color.clone(),
            layout: Some(theme.layout.clone()),
            outline: theme.outline.clone(),
        },
        theme.transform.clone(),
    );
    Ok(commands)
}
