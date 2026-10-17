use sungeki_editor::Command;
use sungeki_project::{Clip, ClipContent};
fn clip_has_timing(clip: &Clip) -> bool {
    matches!(&clip.content, ClipContent::Speech { audio: Some(audio), .. } if audio.lip_sync.is_some())
}
pub(crate) fn command(command: &Command) -> bool {
    match command {
        Command::SetSpeechAudio {
            audio: Some(audio), ..
        } => audio.lip_sync.is_some(),
        Command::AddClip { clip } => clip_has_timing(clip),
        Command::ReplaceProject { project, .. } => project.clips.iter().any(clip_has_timing),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    #[test]
    fn all_timing_edit_paths_require_minor_31() {
        let timing = serde_json::from_str(include_str!(
            "../../../tests/fixtures/lipsync-analysis.json"
        ))
        .unwrap();
        let audio = sungeki_project::SpeechAudio {
            irodori_source: None,
            qwen_source: None,
            sbv2_source: None,
            asset_id: "a".into(),
            input_fingerprint: "b".repeat(64),
            engine_version: "test".into(),
            lip_sync: Some(timing),
        };
        let mut project: sungeki_project::Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        project.clips[0].content = ClipContent::Speech {
            gain_db: None,
            character_id: "c".into(),
            subtitle_text: "字幕".into(),
            spoken_text: None,
            audio: Some(audio.clone()),
        };
        for command in [
            Command::SetSpeechAudio {
                clip_id: "c".into(),
                audio: Some(audio),
            },
            Command::AddClip {
                clip: project.clips[0].clone(),
            },
            Command::ReplaceProject {
                checkpoint_id: None,
                project,
            },
        ] {
            assert_eq!(
                SessionRequest::PreviewEdit {
                    session_id: "s".into(),
                    expected_revision: 0,
                    commands: vec![command.clone()],
                    offset: 0,
                    limit: 10
                }
                .minimum_protocol_minor(),
                31
            );
            assert_eq!(
                SessionRequest::Edit {
                    session_id: "s".into(),
                    edit: EditRequest {
                        operation_id: "o".into(),
                        expected_revision: 0,
                        action: EditAction::Apply {
                            commands: vec![command]
                        }
                    }
                }
                .minimum_protocol_minor(),
                31
            );
        }
    }
}

pub(crate) fn mouth_binding(command: &Command) -> bool {
    let bound = |clip: &Clip| {
        matches!(
            &clip.content,
            ClipContent::Psd {
                speech: Some(_),
                ..
            }
        )
    };
    match command {
        Command::SetPsdSpeech { .. } => true,
        Command::AddClip { clip } => bound(clip),
        Command::ReplaceProject { project, .. } => project.clips.iter().any(bound),
        _ => false,
    }
}

#[cfg(test)]
mod binding_tests {
    use super::*;
    use crate::*;
    #[test]
    fn binding_changes_require_minor_33() {
        let project: sungeki_project::Project = serde_json::from_str(include_str!(
            "../../../tests/fixtures/psd-speech-project.json"
        ))
        .unwrap();
        let ClipContent::Psd { speech, .. } = project.clips[0].content.clone() else {
            panic!()
        };
        for command in [
            Command::SetPsdSpeech {
                clip_id: project.clips[0].id.clone(),
                speech,
            },
            Command::SetPsdSpeech {
                clip_id: project.clips[0].id.clone(),
                speech: None,
            },
            Command::AddClip {
                clip: project.clips[0].clone(),
            },
            Command::ReplaceProject {
                checkpoint_id: None,
                project,
            },
        ] {
            assert_eq!(
                SessionRequest::PreviewEdit {
                    session_id: "s".into(),
                    expected_revision: 0,
                    commands: vec![command.clone()],
                    offset: 0,
                    limit: 10
                }
                .minimum_protocol_minor(),
                33
            );
            assert_eq!(
                SessionRequest::Edit {
                    session_id: "s".into(),
                    edit: EditRequest {
                        operation_id: "o".into(),
                        expected_revision: 0,
                        action: EditAction::Apply {
                            commands: vec![command]
                        }
                    }
                }
                .minimum_protocol_minor(),
                33
            );
        }
    }
}
