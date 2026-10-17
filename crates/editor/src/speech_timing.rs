//! Pure single-utterance timing plan. The host must check the captured speech
//! input and commit these commands with the audio reference in one transaction.
use crate::{Command, apply_batch};
use sungeki_project::{ClipContent, MAX_TIME_US, Project, TimeUs, ValidationError, validate};

#[derive(Debug, Clone, PartialEq)]
pub struct SpeechTimingPlan {
    pub commands: Vec<Command>,
    /// All speech overlaps, including other tracks and muted clips. Half-open
    /// intervals that only touch are not overlaps. No implicit ripple editing.
    pub overlapping_clip_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpeechTimingError {
    Invalid(ValidationError),
    SceneConflict {
        scene_id: String,
        required_end_us: TimeUs,
        conflicting_scene_ids: Vec<String>,
    },
}
impl From<ValidationError> for SpeechTimingError {
    fn from(value: ValidationError) -> Self {
        Self::Invalid(value)
    }
}

pub fn plan_speech_timing(
    project: &Project,
    clip_id: &str,
    duration_us: TimeUs,
) -> Result<SpeechTimingPlan, SpeechTimingError> {
    validate(project)?;
    let clip = project
        .clips
        .iter()
        .find(|c| c.id == clip_id)
        .filter(|c| matches!(c.content, ClipContent::Speech { .. }))
        .ok_or_else(|| ValidationError::new("音声クリップが見つかりません".into()))?;
    let end = clip
        .start_us
        .0
        .checked_add(duration_us.0)
        .filter(|end| duration_us.0 > 0 && *end <= MAX_TIME_US)
        .ok_or_else(|| ValidationError::new("音声の実尺が0〜7日の範囲外です".into()))?;
    let mut commands = vec![Command::ResizeClip {
        clip_id: clip.id.clone(),
        duration_us,
    }];
    if let Some(scene_id) = &clip.scene_id {
        // validate above proves that the scene exists and its end is bounded.
        let scene = project.scenes.iter().find(|s| s.id == *scene_id).unwrap();
        if end > scene.start_us.0 + scene.duration_us.0 {
            let conflicting_scene_ids: Vec<_> = project
                .scenes
                .iter()
                .filter(|other| {
                    other.id != *scene_id
                        && other.start_us.0 < end
                        && scene.start_us.0 < other.start_us.0 + other.duration_us.0
                })
                .map(|other| other.id.clone())
                .collect();
            if !conflicting_scene_ids.is_empty() {
                return Err(SpeechTimingError::SceneConflict {
                    scene_id: scene_id.clone(),
                    required_end_us: TimeUs(end),
                    conflicting_scene_ids,
                });
            }
            commands.push(Command::ResizeScene {
                scene_id: scene_id.clone(),
                duration_us: TimeUs(end - scene.start_us.0),
            });
        }
    }
    // Use the same final validation as every other editor transaction.
    apply_batch(project, &commands)?;
    let overlapping_clip_ids = project
        .clips
        .iter()
        .filter(|other| {
            other.id != clip.id
                && matches!(other.content, ClipContent::Speech { .. })
                && other.start_us.0 < end
                && clip.start_us.0 < other.start_us.0 + other.duration_us.0
        })
        .map(|other| other.id.clone())
        .collect();
    Ok(SpeechTimingPlan {
        commands,
        overlapping_clip_ids,
    })
}
