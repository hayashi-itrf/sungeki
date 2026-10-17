pub(crate) fn command(command: &sungeki_editor::Command) -> bool {
    use sungeki_editor::Command;
    use sungeki_project::{AssetProbe, ClipContent};
    match command {
        Command::SetPsd { .. } => true,
        Command::RegisterAsset { asset } => matches!(asset.probe, AssetProbe::Psd(_)),
        Command::AddClip { clip } => matches!(clip.content, ClipContent::Psd { .. }),
        Command::ReplaceProject { project, .. } => project
            .assets
            .iter()
            .any(|a| matches!(a.probe, AssetProbe::Psd(_))),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn inspection_and_both_edit_paths_require_psd_capable_protocol() {
        use sungeki_editor::{Command, EditAction, EditRequest};
        let project: sungeki_project::Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        let sungeki_project::ClipContent::Psd { asset_id, pose, .. } =
            project.clips[0].content.clone()
        else {
            panic!()
        };
        assert_eq!(
            SessionRequest::ProbePsd {
                path: "/test.psd".into()
            }
            .minimum_protocol_minor(),
            30
        );
        for command in [
            Command::RegisterAsset {
                asset: project.assets[0].clone(),
            },
            Command::AddClip {
                clip: project.clips[0].clone(),
            },
            Command::SetPsd {
                clip_id: project.clips[0].id.clone(),
                asset_id,
                pose,
            },
            Command::ReplaceProject {
                checkpoint_id: None,
                project: project.clone(),
            },
        ] {
            let request = SessionRequest::PreviewEdit {
                session_id: "s".into(),
                expected_revision: 0,
                commands: vec![command.clone()],
                offset: 0,
                limit: 10,
            };
            assert_eq!(request.minimum_protocol_minor(), 30);
            let request = SessionRequest::Edit {
                session_id: "s".into(),
                edit: EditRequest {
                    operation_id: "o".into(),
                    expected_revision: 0,
                    action: EditAction::Apply {
                        commands: vec![command],
                    },
                },
            };
            assert_eq!(request.minimum_protocol_minor(), 30);
        }
    }
}
