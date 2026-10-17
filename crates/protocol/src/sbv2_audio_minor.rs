use sungeki_editor::Command;
use sungeki_project::{Clip, ClipContent};
fn clip(clip: &Clip) -> bool {
    matches!(&clip.content, ClipContent::Speech { audio: Some(audio), .. } if audio.sbv2_source.is_some())
}
pub(crate) fn command(command: &Command) -> bool {
    match command {
        Command::SetSpeechAudio {
            audio: Some(audio), ..
        } => audio.sbv2_source.is_some(),
        Command::AddClip { clip: value } => clip(value),
        Command::ReplaceProject { project, .. } => project.clips.iter().any(clip),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_sbv2_audio_requires_minor_67_on_every_import_path() {
        let mut project: sungeki_project::Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        let audio = sungeki_project::SpeechAudio {
            asset_id:"fixture".into(), input_fingerprint:"a".repeat(64), engine_version:"fixture".into(), lip_sync:None,
            irodori_source: None,
            qwen_source: None,
            sbv2_source: Some(serde_json::from_value(serde_json::json!({
                "selection":{"model_id":0,"speaker_id":0,"speaker_name":"声","style":"Neutral","style_id":0,"metadata_sha256":"a".repeat(64)},
                "identity":{"engine_revision":"fixture","model_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"style_vectors_sha256":"c".repeat(64),"text_encoder_fingerprint":"d".repeat(64)},
                "sample_rate":44100,"channels":1
            })).unwrap()),
        };
        let mut clip = project.clips[0].clone();
        clip.content = ClipContent::Speech {
            gain_db: None,
            character_id: "fixture".into(),
            subtitle_text: "字幕".into(),
            spoken_text: None,
            audio: Some(audio.clone()),
        };
        project.clips = vec![clip.clone()];
        for command in [
            Command::SetSpeechAudio {
                clip_id: clip.id.clone(),
                audio: Some(audio),
            },
            Command::AddClip { clip },
            Command::ReplaceProject {
                project,
                checkpoint_id: None,
            },
        ] {
            let request = crate::SessionRequest::PreviewEdit {
                session_id: "test".into(),
                expected_revision: 0,
                offset: 0,
                limit: 1,
                commands: vec![command],
            };
            assert_eq!(request.minimum_protocol_minor(), 67);
        }
    }
}
