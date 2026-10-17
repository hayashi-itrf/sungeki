//! Required versions for the original shared editing commands.
use crate::SessionRequest;
pub(super) fn minimum(request: &SessionRequest) -> u16 {
    let commands = match request {
        SessionRequest::PreviewEdit { commands, .. } => Some(commands),
        SessionRequest::Edit { edit, .. } => match &edit.action {
            sungeki_editor::EditAction::Apply { commands } => Some(commands),
            _ => None,
        },
        _ => None,
    };
    if commands.is_some_and(|commands| commands.iter().any(crate::sbv2_audio_minor::command)) {
        return 67;
    }
    if commands.is_some_and(|commands| {
        commands.iter().any(|c| match c {
            sungeki_editor::Command::SetCharacter { character } => {
                matches!(character.voice, sungeki_project::Voice::Sbv2 { .. })
            }
            sungeki_editor::Command::ReplaceProject { project, .. } => project
                .characters
                .iter()
                .any(|c| matches!(c.voice, sungeki_project::Voice::Sbv2 { .. })),
            _ => false,
        })
    }) {
        return 66;
    }
    if commands.is_some_and(|commands| {
        commands.iter().any(|c| {
            matches!(c,
        sungeki_editor::Command::PrependForeword { template }
        if template.preset_id == "foreword-portrait-v1")
        })
    }) {
        return 65;
    }
    if commands.is_some_and(|c| c.iter().any(crate::volume_minor::command)) {
        return 61;
    }
    if commands.is_some_and(|commands| commands.iter().any(crate::compressed_audio_minor::command))
    {
        return 60;
    }
    if commands.is_some_and(|commands| commands.iter().any(crate::motion_minor::command)) {
        return 58;
    }
    if commands.is_some_and(|commands| commands.iter().any(crate::scene_fade_minor::command)) {
        return 57;
    }
    if commands.is_some_and(|commands| {
        commands
            .iter()
            .any(|c| matches!(c, sungeki_editor::Command::PrependForeword { .. }))
    }) {
        return 56;
    }
    if commands.is_some_and(|commands| commands.iter().any(crate::blur_minor::command)) {
        return 55;
    }
    if commands.is_some_and(|commands| {
        commands
            .iter()
            .any(|c| matches!(c, sungeki_editor::Command::RefreshSceneCredits { .. }))
    }) {
        return 54;
    }
    if commands.is_some_and(|commands| {
        commands
            .iter()
            .any(|c| matches!(c, sungeki_editor::Command::FitVisualToSpeech { .. }))
    }) {
        return 53;
    }
    if commands.is_some_and(|commands| {
        commands.iter().any(|c| match c {
            sungeki_editor::Command::SetTransform { transform, .. } => {
                transform.flip_x.unwrap_or(false)
            }
            sungeki_editor::Command::AddClip { clip } => clip.transform.flip_x.unwrap_or(false),
            _ => false,
        })
    }) {
        return 52;
    }
    if commands.is_some_and(|commands| {
        commands.iter().any(|c| match c {
            sungeki_editor::Command::SetTransform { transform, .. } => transform.fade.is_some(),
            sungeki_editor::Command::AddClip { clip } => clip.transform.fade.is_some(),
            _ => false,
        })
    }) {
        return 51;
    }
    if commands.is_some_and(|commands| {
        commands
            .iter()
            .any(|c| matches!(c, sungeki_editor::Command::ApplyDialogueTemplate { .. }))
    }) {
        return 50;
    }
    if commands.is_some_and(|commands| {
        commands.iter().any(|c| {
            matches!(c,
                sungeki_editor::Command::AppendSpeechScript { script }
                if script.lines.iter().any(|line| line.character_id.is_some())
            )
        })
    }) {
        return 49;
    }
    if commands.is_some_and(|commands| {
        commands.iter().any(|c| match c {
            sungeki_editor::Command::SetCharacter { character } => {
                character.subtitle_style.layout.is_some()
            }
            sungeki_editor::Command::ReplaceProject { project, .. } => project
                .characters
                .iter()
                .any(|c| c.subtitle_style.layout.is_some()),
            _ => false,
        })
    }) {
        return 48;
    }
    if commands.is_some_and(|commands| {
        commands.iter().any(|c| match c {
            sungeki_editor::Command::SetCharacter { character } => {
                character.subtitle_style.outline.is_some()
            }
            sungeki_editor::Command::ReplaceProject { project, .. } => project
                .characters
                .iter()
                .any(|c| c.subtitle_style.outline.is_some()),
            _ => false,
        })
    }) {
        return 47;
    }
    if commands.is_some_and(|commands| {
        commands
            .iter()
            .any(|c| matches!(c, sungeki_editor::Command::LayoutSpeech { .. }))
    }) {
        return 46;
    }
    if commands.is_some_and(|commands| {
        commands
            .iter()
            .any(|c| matches!(c, sungeki_editor::Command::AppendSpeechScript { .. }))
    }) {
        return 45;
    }
    if commands.is_some_and(|commands| {
        commands
            .iter()
            .any(|command| matches!(command, sungeki_editor::Command::SetAudioMix { .. }))
    }) {
        return 43;
    }
    if commands.is_some_and(|commands| {
        commands.iter().any(|command| {
            use sungeki_editor::Command;
            let audio = |clip: &sungeki_project::Clip| {
                matches!(clip.content, sungeki_project::ClipContent::Audio { .. })
            };
            match command {
                Command::SetAudio { .. } => true,
                Command::AddClip { clip } => audio(clip),
                Command::ReplaceProject { project, .. } => project.clips.iter().any(audio),
                _ => false,
            }
        })
    }) {
        return 42;
    }
    if commands.is_some_and(|commands| {
        commands.iter().any(|command| {
            use sungeki_editor::Command;
            use sungeki_project::ClipContent;
            let has_gain = |clip: &sungeki_project::Clip| {
                matches!(
                    clip.content,
                    ClipContent::Speech {
                        gain_db: Some(_),
                        ..
                    }
                )
            };
            match command {
                Command::SetSpeechGain { .. } => true,
                Command::AddClip { clip } => has_gain(clip),
                Command::ReplaceProject { project, .. } => project.clips.iter().any(has_gain),
                _ => false,
            }
        })
    }) {
        return 37;
    }
    if commands.is_some_and(|commands| commands.iter().any(crate::lipsync_minor::mouth_binding)) {
        return 33;
    }
    if commands.is_some_and(|commands| commands.iter().any(crate::lipsync_minor::command)) {
        return 31;
    }
    if commands.is_some_and(|commands| commands.iter().any(crate::psd_minor::command)) {
        return 30;
    }
    if commands.is_some_and(|commands| {
        commands.iter().any(|c| {
            matches!(
                c,
                sungeki_editor::Command::RegisterAsset { .. }
                    | sungeki_editor::Command::ReplaceProject { .. }
            )
        })
    }) {
        27
    } else if commands
        .is_some_and(|commands| commands.iter().any(crate::speech_minor::speech_command))
    {
        27
    } else if commands.is_some_and(|commands| {
        commands.iter().any(|c| {
            matches!(
                c,
                sungeki_editor::Command::SetPronunciationDictionary { .. }
            )
        })
    }) {
        27
    } else if commands.is_some_and(|commands| {
        commands
            .iter()
            .any(|c| matches!(c, sungeki_editor::Command::SetCanvas { .. }))
    }) {
        18
    } else if commands.is_some_and(|commands| {
        commands.iter().any(|c| {
            matches!(
                c,
                sungeki_editor::Command::SetTextStyle { .. }
                    | sungeki_editor::Command::SetShape { .. }
            )
        })
    }) {
        17
    } else if commands.is_some_and(|commands| {
        commands.iter().any(|c| {
            matches!(
                c,
                sungeki_editor::Command::SetTrackProperties { .. }
                    | sungeki_editor::Command::ReorderTracks { .. }
                    | sungeki_editor::Command::SetClipTrack { .. }
            )
        })
    }) {
        16
    } else if commands.is_some_and(|commands| {
        commands
            .iter()
            .any(|c| matches!(c, sungeki_editor::Command::DuplicateClip { .. }))
    }) {
        15
    } else if commands.is_some_and(|commands| {
        commands
            .iter()
            .any(|c| matches!(c, sungeki_editor::Command::ResizeClip { .. }))
    }) {
        14
    } else if commands.is_some_and(|commands| {
        commands
            .iter()
            .any(|c| matches!(c, sungeki_editor::Command::SetAssetMetadata { .. }))
    }) {
        13
    } else if matches!(request, SessionRequest::ImagePreview { .. }) {
        12
    } else {
        11
    }
}
