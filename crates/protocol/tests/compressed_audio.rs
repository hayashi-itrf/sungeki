use sungeki_editor::Command;
use sungeki_project::*;
use sungeki_protocol::SessionRequest;
#[test]
fn registering_and_replacing_compressed_sources_require_minor_60() {
    let a: Asset = serde_json::from_value(serde_json::json!({
        "id":"a", "name":"MP3", "source":null, "credit":"test", "content_hash":null,
        "locator":{"type":"absolute","path":"/source.mp3"},
        "probe":{"type":"audio","info":{"sha256":"a".repeat(64),"byte_length":1000,
            "frames":48000,"sample_rate":48000,"channels":2,"duration_us":1000000,
            "encoding":{"type":"mp3","decode_version":1,"decoded_sha256":"b".repeat(64)}}}
    }))
    .unwrap();
    let mut project = Project::empty("p".into(), "test".into());
    project.assets.push(a.clone());
    for command in [
        Command::RegisterAsset { asset: a },
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
        assert_eq!(request.minimum_protocol_minor(), 60);
    }
}

#[test]
fn mp3_inspection_requires_minor_60_but_wav_does_not() {
    for path in ["/source.mp3", "/source.MP3"] {
        assert_eq!(
            SessionRequest::ProbeAudio { path: path.into() }.minimum_protocol_minor(),
            60
        );
    }
    assert_eq!(
        SessionRequest::ProbeAudio {
            path: "/source.wav".into()
        }
        .minimum_protocol_minor(),
        41
    );
}
