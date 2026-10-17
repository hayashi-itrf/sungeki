use sungeki_editor::Command;
use sungeki_project::*;
use sungeki_protocol::SessionRequest;
#[test]
fn commands_and_embedded_scenes_require_minor_57() {
    let scene = Scene {
        id: "s".into(),
        name: "s".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(1),
        fade: Some(VisualFade {
            in_us: TimeUs(1),
            out_us: TimeUs(0),
        }),
    };
    let mut project = Project::empty("p".into(), "p".into());
    project.scenes.push(scene.clone());
    for command in [
        Command::SetSceneFade {
            scene_id: "s".into(),
            fade: None,
        },
        Command::AddScene { scene },
        Command::ReplaceProject {
            checkpoint_id: None,
            project,
        },
    ] {
        let request = SessionRequest::PreviewEdit {
            session_id: "s".into(),
            expected_revision: 0,
            commands: vec![command],
            offset: 0,
            limit: 20,
        };
        assert_eq!(request.minimum_protocol_minor(), 57);
    }
}
