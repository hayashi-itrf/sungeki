use sungeki_editor::{Command, EditAction, EditRequest};
use sungeki_project::{AudioMix, Normalization};
use sungeki_protocol::SessionRequest;
#[test]
fn set_and_clear_mix_require_minor_43_for_apply_and_preview() {
    for normalization in [
        None,
        Some(Normalization {
            target_lufs: -18.0,
            true_peak_db: -1.0,
        }),
    ] {
        let commands = vec![Command::SetAudioMix {
            audio_mix: AudioMix { normalization },
        }];
        let requests = [
            SessionRequest::PreviewEdit {
                session_id: "s".into(),
                expected_revision: 0,
                commands: commands.clone(),
                offset: 0,
                limit: 10,
            },
            SessionRequest::Edit {
                session_id: "s".into(),
                edit: EditRequest {
                    expected_revision: 0,
                    operation_id: "o".into(),
                    action: EditAction::Apply { commands },
                },
            },
        ];
        for request in requests {
            assert_eq!(request.minimum_protocol_minor(), 43);
        }
    }
}
