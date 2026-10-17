//! Compose only the speech staging document. Visuals and real-duration layout
//! are separate build stages; this document is not ready for rendering.
use crate::{Command, SpeechScript, SpeechScriptLine, apply_batch};
use serde::{Deserialize, Serialize};
use sungeki_project::*;

/// Allocate once and persist with the parent job before applying its batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildScriptIds {
    pub track_id: String,
    pub scene_ids: Vec<String>,
    /// Flattened in recipe scene/line order.
    pub speech_ids: Vec<String>,
}

/// Pure command planning for an untouched new session. Retry the original
/// EditRequest, not this planner against an already composed project.
pub fn plan_build_script(
    project: &Project,
    recipe: &BuildRecipe,
    ids: &BuildScriptIds,
) -> Result<Vec<Command>, ValidationError> {
    recipe.validate()?;
    if *project != Project::empty(project.id.clone(), project.title.clone()) {
        return Err(ValidationError::new(
            "制作台本は未編集の新規作品に作成してください".into(),
        ));
    }
    let count: usize = recipe.scenes.iter().map(|s| s.lines.len()).sum();
    if ids.scene_ids.len() != recipe.scenes.len() || ids.speech_ids.len() != count {
        return Err(ValidationError::new(
            "制作台本の固定IDとシーン・発話数が一致しません".into(),
        ));
    }
    let mut commands = vec![
        Command::RenameProject {
            title: recipe.title.clone(),
        },
        Command::SetCanvas {
            canvas: Canvas {
                width: if recipe.preset_id == "narration-portrait-v1" {
                    1080
                } else {
                    1920
                },
                height: if recipe.preset_id == "narration-portrait-v1" {
                    1920
                } else {
                    1080
                },
                fps: FrameRate {
                    numerator: 30,
                    denominator: 1,
                },
                background: "#000000".into(),
            },
        },
        Command::SetAudioMix {
            audio_mix: recipe.audio_mix.clone(),
        },
        Command::SetPronunciationDictionary {
            entries: recipe.pronunciation_dictionary.clone(),
        },
        Command::AddTrack {
            track: Track {
                id: ids.track_id.clone(),
                name: "セリフ".into(),
                kind: TrackKind::Audio,
                visible: true,
                muted: false,
            },
        },
    ];
    commands.extend(
        recipe
            .assets
            .iter()
            .cloned()
            .map(|asset| Command::RegisterAsset { asset }),
    );
    commands.extend(recipe.roles.iter().map(|role| {
        let mut character = role.character.clone();
        // Templates place subtitles inside bounded rectangles. Unspecified
        // layout uses wrapping; explicitly chosen layout remains authoritative.
        character
            .subtitle_style
            .layout
            .get_or_insert(SubtitleLayout {
                alignment: TextAlignment::Left,
                wrap: true,
            });
        let outline_color = subtitle_outline_color(&character.subtitle_style.color);
        character
            .subtitle_style
            .outline
            .get_or_insert(SubtitleOutline {
                width: 3,
                color: outline_color.into(),
            });
        Command::SetCharacter { character }
    }));
    let mut cursor = 0;
    let mut line_offset = 0;
    for (index, scene) in recipe.scenes.iter().enumerate() {
        // These lengths are explicitly provisional. Synthesis must reflow all
        // affected scenes atomically before applying an audio result.
        let duration =
            scene.lines.len() as u64 * 1_000_000 + (scene.lines.len() as u64 - 1) * scene.gap_us.0;
        let scene_id = &ids.scene_ids[index];
        commands.push(Command::AddScene {
            scene: Scene {
                id: scene_id.clone(),
                name: scene.name.clone(),
                start_us: TimeUs(cursor),
                duration_us: TimeUs(duration),
                fade: None,
            },
        });
        let lines = scene
            .lines
            .iter()
            .enumerate()
            .map(|(offset, line)| {
                let role = recipe
                    .roles
                    .iter()
                    .find(|role| role.role == line.role)
                    .expect("validated recipe role");
                SpeechScriptLine {
                    character_id: Some(role.character.id.clone()),
                    clip_id: ids.speech_ids[line_offset + offset].clone(),
                    subtitle_text: line.subtitle_text.clone(),
                    spoken_text: line.spoken_text.clone(),
                }
            })
            .collect();
        commands.push(Command::AppendSpeechScript {
            script: SpeechScript {
                lines,
                character_id: recipe.roles[0].character.id.clone(),
                track_id: ids.track_id.clone(),
                scene_id: Some(scene_id.clone()),
                start_us: TimeUs(cursor),
                provisional_duration_us: TimeUs(1_000_000),
                gap_us: scene.gap_us,
            },
        });
        cursor += duration;
        line_offset += scene.lines.len();
    }
    // Validate IDs, scene bounds and every command through the shared editor.
    // This candidate is never persisted by the planner.
    let candidate = apply_batch(project, &commands)?;
    // Geometry is known before synthesis; validate with the same placement used
    // by final layout, without guessing any speech duration.
    for (scene_index, scene) in recipe.scenes.iter().enumerate() {
        for (diagram_index, diagram) in scene.diagrams.iter().enumerate() {
            if let Some(motion) = &diagram.motion {
                let start = crate::build_visuals::diagram_rect(&candidate, &diagram.asset_id)?;
                motion.validate(&start).map_err(|e| {
                    ValidationError::new(format!(
                        "シーン{}・説明図{}: {}",
                        scene_index + 1,
                        diagram_index + 1,
                        e.0
                    ))
                })?;
            }
        }
    }
    Ok(commands)
}

fn subtitle_outline_color(color: &str) -> &'static str {
    let rgb = u32::from_str_radix(&color[1..], 16).expect("validated subtitle color");
    let brightness = ((rgb >> 16) * 299 + ((rgb >> 8) & 255) * 587 + (rgb & 255) * 114) / 1000;
    if brightness < 128 {
        "#ffffff"
    } else {
        "#000000"
    }
}
