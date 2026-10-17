use crate::SessionRequest;
use sungeki_editor::{Command, EditAction};
use sungeki_project::{Asset, AssetProbe};
pub(super) fn build_required(request: &SessionRequest) -> bool {
    matches!(request, SessionRequest::BuildVideo { request: crate::BuildVideoRequest::Start { args } }
        if args.recipe.assets.iter().any(vrm) || args.recipe.roles.iter().any(|r| matches!(r.standing, sungeki_project::BuildStanding::Vrm { .. })))
}
fn vrm(asset: &Asset) -> bool {
    matches!(asset.probe, AssetProbe::Vrm(_))
}
fn commands(items: &[Command]) -> bool {
    items.iter().any(|c| match c {
        Command::RegisterAsset { asset } => vrm(asset),
        Command::ReplaceProject { project, .. } => project.assets.iter().any(vrm),
        _ => false,
    })
}
pub(super) fn required(request: &SessionRequest) -> bool {
    match request {
        SessionRequest::Edit { edit, .. } => match &edit.action {
            EditAction::Apply { commands: items } => commands(items),
            _ => false,
        },
        SessionRequest::PreviewEdit {
            commands: items, ..
        } => commands(items),
        SessionRequest::BuildVideo {
            request: crate::BuildVideoRequest::Start { args },
        } => args.recipe.assets.iter().any(vrm),
        _ => false,
    }
}

pub(super) fn clip_required(request: &SessionRequest) -> bool {
    let items = match request {
        SessionRequest::Edit { edit, .. } => match &edit.action {
            EditAction::Apply { commands } => commands,
            _ => return false,
        },
        SessionRequest::PreviewEdit { commands, .. } => commands,
        _ => return false,
    };
    items.iter().any(|command| match command {
        Command::SetVrm { .. } => true,
        Command::AddClip { clip } => {
            matches!(clip.content, sungeki_project::ClipContent::Vrm { .. })
        }
        Command::ReplaceProject { project, .. } => project
            .clips
            .iter()
            .any(|c| matches!(c.content, sungeki_project::ClipContent::Vrm { .. })),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vrm_build_requires_minor_97_before_asset_minor_93() {
        let p: sungeki_project::Project = serde_json::from_str(include_str!(
            "../../../tests/fixtures/vrm-clip-project.json"
        ))
        .unwrap();
        let mut recipe: sungeki_project::BuildRecipe = serde_json::from_str(include_str!(
            "../../../tests/fixtures/recipes/build-dialogue.json"
        ))
        .unwrap();
        recipe.assets.extend(p.assets);
        let request = SessionRequest::BuildVideo {
            request: crate::BuildVideoRequest::Start {
                args: crate::StartBuildVideoArgs {
                    wait_ms: None,
                    reuse_from: None,
                    managed_profiles: Default::default(),
                    job_id: "j".into(),
                    recipe,
                    project_path: "/tmp/test.vvm.json".into(),
                    base_url: "http://127.0.0.1:50021".into(),
                    engine_version: "test".into(),
                },
            },
        };
        assert_eq!(request.minimum_protocol_minor(), 97);
    }
    #[test]
    fn vrm_clip_requires_minor_95() {
        let project: sungeki_project::Project = serde_json::from_str(include_str!(
            "../../../tests/fixtures/vrm-clip-project.json"
        ))
        .unwrap();
        let request = SessionRequest::PreviewEdit {
            session_id: "s".into(),
            expected_revision: 0,
            commands: vec![Command::AddClip {
                clip: project.clips[0].clone(),
            }],
            offset: 0,
            limit: 10,
        };
        assert_eq!(request.minimum_protocol_minor(), 95);
    }
    #[test]
    fn vrm_probe_requires_minor_94() {
        assert_eq!(
            SessionRequest::ProbeVrm {
                path: "/model.vrm".into(),
                kind: sungeki_project::VrmKind::Model
            }
            .minimum_protocol_minor(),
            94
        );
    }
    #[test]
    fn vrm_metadata_requires_minor_93() {
        let project: sungeki_project::Project = serde_json::from_str(include_str!(
            "../../../tests/fixtures/vrm-assets-project.json"
        ))
        .unwrap();
        let request = SessionRequest::PreviewEdit {
            session_id: "s".into(),
            expected_revision: 0,
            commands: vec![Command::RegisterAsset {
                asset: project.assets[0].clone(),
            }],
            offset: 0,
            limit: 10,
        };
        assert_eq!(request.minimum_protocol_minor(), 93);
    }
}

pub(super) fn pose_required(request: &SessionRequest) -> bool {
    fn clip_pose(clip: &sungeki_project::Clip) -> bool {
        matches!(&clip.content, sungeki_project::ClipContent::Vrm { standing } if standing.pose.is_some())
    }
    let commands = match request {
        SessionRequest::Edit { edit, .. } => match &edit.action {
            EditAction::Apply { commands } => commands,
            _ => return false,
        },
        SessionRequest::PreviewEdit { commands, .. } => commands,
        SessionRequest::BuildVideo {
            request: crate::BuildVideoRequest::Start { args },
        } => {
            return args.recipe.roles.iter().any(|r| {
                matches!(&r.standing,
                sungeki_project::BuildStanding::Vrm { standing, .. } if standing.pose.is_some())
            });
        }
        _ => return false,
    };
    commands.iter().any(|c| match c {
        Command::SetVrm { standing, .. } => standing.pose.is_some(),
        Command::AddClip { clip } => clip_pose(clip),
        Command::ReplaceProject { project, .. } => project.clips.iter().any(clip_pose),
        _ => false,
    })
}
