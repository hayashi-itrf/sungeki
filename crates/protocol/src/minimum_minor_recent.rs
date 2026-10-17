#[path = "minimum_minor_playback.rs"]
mod playback;
#[path = "minimum_minor_text.rs"]
mod text;
use crate::SessionRequest;
#[path = "minimum_minor_irodori.rs"]
mod irodori;
pub(super) fn minimum(request: &SessionRequest) -> Option<u16> {
    if matches!(request, SessionRequest::InstallSpeechConfig { .. }) {
        return Some(119);
    }
    if text::outlines(request) {
        return Some(118);
    }
    if text::edits_layout(request) {
        return Some(117);
    }
    if let Some(minor) = playback::minimum(request) {
        return Some(minor);
    }
    if matches!(request, SessionRequest::BuildVideo { request: crate::BuildVideoRequest::Start { args } }
        if args.recipe.scenes.iter().any(|s| s.diagrams.iter().any(|d| d.reaction.is_some())))
    {
        return Some(108);
    }

    if text::required(request) {
        return Some(107);
    }
    if matches!(request, SessionRequest::BuildVideo { request: crate::BuildVideoRequest::Start { args } }
        if !args.managed_profiles.is_empty())
    {
        return Some(106);
    }
    if irodori::audio_required(request) {
        return Some(104);
    }
    if irodori::required(request) {
        return Some(103);
    }
    let reaction_commands = match request {
        SessionRequest::Edit { edit, .. } => match &edit.action {
            sungeki_editor::EditAction::Apply { commands } => Some(commands),
            _ => None,
        },
        SessionRequest::PreviewEdit { commands, .. } => Some(commands),
        _ => None,
    };
    if reaction_commands.is_some_and(|commands| {
        commands.iter().any(|c| match c {
            sungeki_editor::Command::SetSpeechAudio { audio: Some(a), .. } => {
                a.qwen_source.is_some()
            }
            sungeki_editor::Command::AddClip { clip } => qwen_audio(clip),
            sungeki_editor::Command::ReplaceProject { project, .. } => {
                project.clips.iter().any(qwen_audio)
            }
            _ => false,
        })
    }) {
        return Some(100);
    }
    if reaction_commands.is_some_and(|commands| {
        commands.iter().any(|command| match command {
            sungeki_editor::Command::SetCharacter { character } => {
                matches!(character.voice, sungeki_project::Voice::Qwen { .. })
            }
            sungeki_editor::Command::ReplaceProject { project, .. } => project
                .characters
                .iter()
                .any(|c| matches!(c.voice, sungeki_project::Voice::Qwen { .. })),
            _ => false,
        })
    }) {
        return Some(99);
    }
    if reaction_commands.is_some_and(|commands| {
        commands
            .iter()
            .any(|command| matches!(command, sungeki_editor::Command::SetReactionMotion { .. }))
    }) {
        return Some(98);
    }
    if crate::minimum_minor_vrm::build_required(request) {
        return Some(97);
    }
    if crate::minimum_minor_vrm::clip_required(request) {
        return Some(95);
    }
    if matches!(request, SessionRequest::ProbeVrm { .. }) {
        return Some(94);
    }
    if crate::minimum_minor_vrm::required(request) {
        return Some(93);
    }
    if let SessionRequest::BuildVideo {
        request: crate::BuildVideoRequest::Start { args },
    } = request
    {
        if args
            .recipe
            .scenes
            .iter()
            .any(|scene| scene.lines.iter().any(|line| line.sprite_pose.is_some()))
        {
            return Some(92);
        }
        if args
            .recipe
            .roles
            .iter()
            .any(|role| matches!(role.standing, sungeki_project::BuildStanding::Sprite { .. }))
        {
            return Some(91);
        }
    }
    if matches!(
        request,
        SessionRequest::Proxy {
            request: crate::ProxyRequest::Remove { .. }
        }
    ) {
        return Some(89);
    }
    if matches!(request, SessionRequest::Proxy { .. }) {
        return Some(87);
    }
    use sungeki_editor::{Command, EditAction};
    let commands = match request {
        SessionRequest::Edit { edit, .. } => match &edit.action {
            EditAction::Apply { commands } => Some(commands),
            _ => None,
        },
        SessionRequest::PreviewEdit { commands, .. } => Some(commands),
        _ => None,
    };
    if commands.is_some_and(|commands| {
        commands.iter().any(|command| match command {
            Command::SetSprite { .. } => true,
            Command::AddClip { clip } => {
                matches!(clip.content, sungeki_project::ClipContent::Sprite { .. })
            }
            Command::ReplaceProject { project, .. } => project
                .clips
                .iter()
                .any(|clip| matches!(clip.content, sungeki_project::ClipContent::Sprite { .. })),
            _ => false,
        })
    }) {
        return Some(90);
    }
    if commands.is_some_and(|commands| {
        commands.iter().any(|command| match command {
            Command::SetVideo { .. } => true,
            Command::AddClip { clip } => {
                matches!(clip.content, sungeki_project::ClipContent::Video { .. })
            }
            Command::ReplaceProject { project, .. } => project
                .clips
                .iter()
                .any(|c| matches!(c.content, sungeki_project::ClipContent::Video { .. })),
            _ => false,
        })
    }) {
        return Some(84);
    }
    let video = |asset: &sungeki_project::Asset| {
        matches!(asset.probe, sungeki_project::AssetProbe::Video(_))
    };
    if commands.is_some_and(|commands| {
        commands.iter().any(|command| match command {
            Command::RegisterAsset { asset } => video(asset),
            Command::ReplaceProject { project, .. } => project.assets.iter().any(video),
            _ => false,
        })
    }) {
        return Some(82);
    }
    match request {
        SessionRequest::BuildVideo {
            request: crate::BuildVideoRequest::Start { args },
        } if args.recipe.assets.iter().any(video)
            || args
                .recipe
                .scenes
                .iter()
                .any(|s| s.diagrams.iter().any(|d| d.video_playback.is_some())) =>
        {
            Some(86)
        }
        SessionRequest::VideoPreview { args } => {
            Some(if args.proxy_job_id.is_some() { 88 } else { 85 })
        }
        SessionRequest::ProbeVideo { .. } => Some(83),
        SessionRequest::Open {
            import_checkpoints: true,
            ..
        } => Some(81),
        SessionRequest::Collection { .. } => Some(80),
        SessionRequest::SpeechJob { request } => Some(request.minimum_protocol_minor()),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn import_is_explicit_and_version_gated() {
        let ordinary: crate::SessionRequest = serde_json::from_value(
            serde_json::json!({"type":"open", "path":"/tmp/project.vvm.json"}),
        )
        .unwrap();
        assert_eq!(
            crate::SessionRequest::ProbeVideo {
                path: "/movie.mp4".into()
            }
            .minimum_protocol_minor(),
            83
        );
        assert!(ordinary.minimum_protocol_minor() < 81);
        let imported: crate::SessionRequest = serde_json::from_value(serde_json::json!({"type":"open", "path":"/tmp/project.vvm.json", "import_checkpoints":true})).unwrap();
        assert_eq!(imported.minimum_protocol_minor(), 81);
    }
}
#[cfg(test)]
mod video_tests {
    use super::*;
    #[test]
    fn video_registration_and_project_replacement_require_minor_82() {
        use sungeki_editor::{Command, EditAction, EditRequest};
        let p: sungeki_project::Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json"))
                .unwrap();
        for command in [
            Command::RegisterAsset {
                asset: p.assets[0].clone(),
            },
            Command::ReplaceProject {
                checkpoint_id: None,
                project: p,
            },
        ] {
            let preview = SessionRequest::PreviewEdit {
                session_id: "s".into(),
                expected_revision: 0,
                commands: vec![command.clone()],
                offset: 0,
                limit: 10,
            };
            assert_eq!(preview.minimum_protocol_minor(), 82);
            let edit = SessionRequest::Edit {
                session_id: "s".into(),
                edit: EditRequest {
                    operation_id: "o".into(),
                    expected_revision: 0,
                    action: EditAction::Apply {
                        commands: vec![command],
                    },
                },
            };
            assert_eq!(edit.minimum_protocol_minor(), 82);
        }
    }
}
#[cfg(test)]
mod video_clip_tests {
    use super::*;
    #[test]
    fn video_clip_add_trim_and_restore_require_minor_84() {
        use sungeki_editor::Command;
        let project: sungeki_project::Project = serde_json::from_str(include_str!(
            "../../../tests/fixtures/video-clip-project.json"
        ))
        .unwrap();
        let clip = project.clips[0].clone();
        let sungeki_project::ClipContent::Video { asset_id, playback } = clip.content.clone()
        else {
            panic!()
        };
        for command in [
            Command::AddClip { clip: clip.clone() },
            Command::SetVideo {
                clip_id: clip.id,
                asset_id,
                playback,
            },
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
                limit: 20,
            };
            assert_eq!(request.minimum_protocol_minor(), 84);
        }
    }
}

#[cfg(test)]
mod proxy_preview_tests {
    #[test]
    fn explicit_proxy_requires_minor_88() {
        let mut value = serde_json::json!({"type":"video_preview", "args":{
            "session_id":"s", "expected_revision":0, "clip_id":"c", "time_us":0
        }});
        let request: crate::SessionRequest = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 85);
        value["args"]["proxy_job_id"] = serde_json::json!("job");
        let request: crate::SessionRequest = serde_json::from_value(value).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 88);
    }
}

#[cfg(test)]
mod sprite_tests {
    use super::*;
    use sungeki_editor::Command;
    #[test]
    fn sprite_add_edit_and_restore_require_minor_90() {
        let p: sungeki_project::Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/sprite-project.json"))
                .unwrap();
        let clip = p.clips[0].clone();
        let sungeki_project::ClipContent::Sprite {
            sprites,
            pose,
            character_id,
        } = clip.content.clone()
        else {
            panic!()
        };
        for command in [
            Command::AddClip { clip: clip.clone() },
            Command::SetSprite {
                clip_id: clip.id,
                sprites,
                pose,
                character_id,
            },
            Command::ReplaceProject {
                project: p,
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
            assert_eq!(request.minimum_protocol_minor(), 90);
        }
    }
}

#[cfg(test)]
mod sprite_recipe_tests {
    #[test]
    fn sprite_build_requires_minor_91() {
        let mut recipe: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/recipes/build-dialogue.json"
        ))
        .unwrap();
        recipe["roles"][0]["standing"] = serde_json::json!({"type":"sprite","pose":null,"flip_x":false,
            "sprites":{"default_pose":"通常","poses":[{"name":"通常","closed_asset_id":"a","open_asset_id":null}]}});
        let request: crate::SessionRequest = serde_json::from_value(serde_json::json!({"type":"build_video","request":{
            "type":"start","args":{"job_id":"j","recipe":recipe,"project_path":"/p.json","base_url":"http://localhost:50021","engine_version":"test"}}})).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 91);
        let crate::SessionRequest::BuildVideo {
            request: crate::BuildVideoRequest::Start { mut args },
        } = request
        else {
            panic!()
        };
        args.recipe.scenes[0].lines[0].sprite_pose = Some("通常".into());
        let changed = crate::SessionRequest::BuildVideo {
            request: crate::BuildVideoRequest::Start { args },
        };
        assert_eq!(changed.minimum_protocol_minor(), 92);
    }
}

fn qwen_audio(clip: &sungeki_project::Clip) -> bool {
    matches!(&clip.content,sungeki_project::ClipContent::Speech{audio:Some(a),..} if a.qwen_source.is_some())
}

#[cfg(test)]
#[path = "build_reaction_tests.rs"]
mod build_reaction_tests;
