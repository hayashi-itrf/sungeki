use crate::{BuildVideoRequest, SessionRequest};
use sungeki_editor::{Command, EditAction};
use sungeki_project::{Character, Voice};
fn contains(character: &Character) -> bool {
    matches!(character.voice, Voice::Irodori { .. })
}
pub(super) fn audio_required(request: &SessionRequest) -> bool {
    let commands = match request {
        SessionRequest::Edit { edit, .. } => match &edit.action {
            EditAction::Apply { commands } => commands,
            _ => return false,
        },
        SessionRequest::PreviewEdit { commands, .. } => commands,
        _ => return false,
    };
    let clip_has_source = |clip: &sungeki_project::Clip| {
        matches!(&clip.content, sungeki_project::ClipContent::Speech { audio: Some(audio), .. }
            if audio.irodori_source.is_some())
    };
    commands.iter().any(|command| match command {
        Command::SetSpeechAudio {
            audio: Some(audio), ..
        } => audio.irodori_source.is_some(),
        Command::AddClip { clip } => clip_has_source(clip),
        Command::ReplaceProject { project, .. } => project.clips.iter().any(clip_has_source),
        _ => false,
    })
}
pub(super) fn required(request: &SessionRequest) -> bool {
    let commands = match request {
        SessionRequest::Edit { edit, .. } => match &edit.action {
            EditAction::Apply { commands } => commands,
            _ => return false,
        },
        SessionRequest::PreviewEdit { commands, .. } => commands,
        SessionRequest::BuildVideo {
            request: BuildVideoRequest::Start { args },
        } => {
            return args
                .recipe
                .roles
                .iter()
                .any(|role| contains(&role.character));
        }
        _ => return false,
    };
    commands.iter().any(|command| match command {
        Command::SetCharacter { character } => contains(character),
        Command::ReplaceProject { project, .. } => project.characters.iter().any(contains),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn recipe() -> sungeki_project::BuildRecipe {
        let mut value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/recipes/build-dialogue.json"
        ))
        .unwrap();
        value["roles"][0]["character"]["voice"] = json!({"type":"irodori",
            "settings":{"voice_id":"registered","caption":null,"cfg_scale":3,"num_steps":40,"seed":42},
            "identity":{"checkpoint":"model","model_manifest_sha256":"a".repeat(64),
                "codec_manifest_sha256":"b".repeat(64),"engine_manifest_sha256":"c".repeat(64),
                "voice_manifest_sha256":"d".repeat(64),"sample_rate":48000,"channels":1}});
        serde_json::from_value(value).unwrap()
    }
    #[test]
    fn captured_audio_add_set_and_restore_require_minor_104() {
        use sungeki_project::*;
        let character = recipe().roles[0].character.clone();
        let Voice::Irodori { settings, identity } = character.voice else {
            panic!()
        };
        let audio = SpeechAudio {
            irodori_source: Some(IrodoriAudioSource {
                settings,
                identity,
                metadata_sha256: "e".repeat(64),
            }),
            qwen_source: None,
            sbv2_source: None,
            asset_id: "a".into(),
            input_fingerprint: "f".repeat(64),
            engine_version: "c".repeat(64),
            lip_sync: None,
        };
        let mut project: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        let mut clip = project.clips[0].clone();
        clip.content = ClipContent::Speech {
            character_id: "c".into(),
            subtitle_text: "字幕".into(),
            spoken_text: None,
            audio: Some(audio.clone()),
            gain_db: None,
        };
        project.clips.push(clip.clone());
        for command in [
            Command::SetSpeechAudio {
                clip_id: "c".into(),
                audio: Some(audio),
            },
            Command::AddClip { clip },
            Command::ReplaceProject {
                project,
                checkpoint_id: None,
            },
        ] {
            let request = SessionRequest::PreviewEdit {
                session_id: "s".into(),
                expected_revision: 0,
                commands: vec![command],
                offset: 0,
                limit: 10,
            };
            assert_eq!(request.minimum_protocol_minor(), 104);
        }
    }
    #[test]
    fn character_edit_restore_and_recipe_require_minor_103() {
        let recipe = recipe();
        let character = recipe.roles[0].character.clone();
        let mut project: sungeki_project::Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        project.characters.push(character.clone());
        for command in [
            Command::SetCharacter { character },
            Command::ReplaceProject {
                project,
                checkpoint_id: None,
            },
        ] {
            let request = SessionRequest::PreviewEdit {
                session_id: "s".into(),
                expected_revision: 0,
                commands: vec![command],
                offset: 0,
                limit: 10,
            };
            assert_eq!(request.minimum_protocol_minor(), 103);
        }
        let request = SessionRequest::BuildVideo {
            request: BuildVideoRequest::Start {
                args: crate::StartBuildVideoArgs {
                    wait_ms: None,
                    reuse_from: None,
                    managed_profiles: Default::default(),
                    job_id: "j".into(),
                    recipe,
                    project_path: "/p.json".into(),
                    base_url: "http://127.0.0.1:50021".into(),
                    engine_version: "v".into(),
                },
            },
        };
        assert_eq!(request.minimum_protocol_minor(), 103);
    }
}
