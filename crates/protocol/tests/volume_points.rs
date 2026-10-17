use sungeki_editor::Command;
use sungeki_project::{ClipContent, Project, TimeUs, VolumePoint};
use sungeki_protocol::SessionRequest;
#[test]
fn every_volume_point_edit_requires_minor_61() {
    let mut project: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/audio-project.json")).unwrap();
    let clip = &mut project.clips[0];
    let ClipContent::Audio { asset_id, playback } = &mut clip.content else {
        panic!()
    };
    playback.volume_points = vec![VolumePoint {
        at_us: TimeUs(0),
        level: 0.0,
    }];
    let set = Command::SetAudio {
        clip_id: clip.id.clone(),
        asset_id: asset_id.clone(),
        playback: playback.clone(),
    };
    for command in [
        set,
        Command::AddClip { clip: clip.clone() },
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
        assert_eq!(request.minimum_protocol_minor(), 61);
    }
}
