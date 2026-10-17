use super::{Input, create_input, decode_input, sbv2_tests::sbv2};
use crate::{
    speech_dispatch::Reservation,
    speech_journal_tests::{Fixture, id},
    speech_state::Application,
};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use sungeki_project::{ClipContent, JobPhase};
use sungeki_protocol::{EditAction, EditRequest, SessionRequest};
use sungeki_speech::SpeechResponse;

#[tokio::test]
async fn sbv2_result_applies_atomically_or_preserves_changed_dialogue_and_pending_original() {
    for changed in [false, true] {
        let fixture = Fixture::new();
        let mut sessions = fixture.open();
        let (sid, input) = sbv2(&mut sessions);
        let job = create_input(&mut sessions, id(), sid.clone(), &input).unwrap();
        Reservation::acquire(&mut sessions, &job.id)
            .unwrap()
            .begin(&mut sessions)
            .unwrap()
            .confirm_finished(&mut sessions)
            .unwrap();
        let Input::Sbv2(input) = input else { panic!() };
        let prepared = input.prepared().unwrap();
        if changed {
            let mut project = input.project().clone();
            let ClipContent::Speech { subtitle_text, .. } = &mut project.clips[0].content else {
                panic!()
            };
            *subtitle_text = "合成中に編集したセリフ".into();
            sessions
                .request(SessionRequest::Edit {
                    session_id: sid.clone(),
                    edit: EditRequest {
                        operation_id: id(),
                        expected_revision: 2,
                        action: EditAction::Apply {
                            commands: vec![sungeki_editor::Command::ReplaceProject {
                                project,
                                checkpoint_id: None,
                            }],
                        },
                    },
                })
                .unwrap();
        }
        let shared = Arc::new(Mutex::new(sessions));
        assert!(
            crate::speech_output::complete(
                &shared,
                &job.id,
                crate::speech_output_tests::response(),
                &AtomicBool::new(false)
            )
            .await
            .is_err()
        );
        let mut bytes = crate::speech_output_tests::response().unverified_wav;
        bytes[24..28].copy_from_slice(&44100u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&88200u32.to_le_bytes());
        crate::speech_output::complete(
            &shared,
            &job.id,
            SpeechResponse::sbv2(prepared.audio(), bytes.clone()).unwrap(),
            &AtomicBool::new(false),
        )
        .await
        .unwrap();
        drop(shared);
        let sessions = fixture.open();
        let record = sessions.store.load_job(&job.id).unwrap().unwrap();
        let (_, state) = decode_input(&record).unwrap();
        assert_eq!(state.phase, JobPhase::Succeeded);
        assert_eq!(std::fs::read(&state.artifact.unwrap().path).unwrap(), bytes);
        let (_, session) = sessions.load(&sid).unwrap();
        let project = session.history().project();
        let ClipContent::Speech {
            audio,
            subtitle_text,
            ..
        } = &project.clips[0].content
        else {
            panic!()
        };
        if changed {
            assert!(matches!(state.application, Application::PendingConflict));
            assert_eq!(subtitle_text, "合成中に編集したセリフ");
            assert!(audio.is_none());
            assert!(project.assets.is_empty());
        } else {
            assert!(matches!(
                state.application,
                Application::Applied { revision: 3, .. }
            ));
            assert_eq!(
                audio.as_ref().unwrap().input_fingerprint,
                prepared.audio().fingerprint()
            );
            assert_eq!(
                audio.as_ref().unwrap().engine_version,
                input.identity().engine_revision
            );
            assert_eq!(project.clips[0].duration_us.0, 545);
            assert_eq!(project.assets.len(), 1);
            assert!(audio.as_ref().unwrap().lip_sync.is_some());
        }
        let baseline: sungeki_project::Project =
            serde_json::from_slice(&serde_json::to_vec(project).unwrap()).unwrap();
        assert_eq!(
            sungeki_speech::audio_reference_matches(&baseline, input.clip_id()).unwrap(),
            !changed
        );

        let shared = Arc::new(Mutex::new(sessions));
        assert_eq!(job_status(&shared, &job.id).stale, changed);
        if !changed {
            for mutation in 0..3 {
                let mut broken = baseline.clone();
                let ClipContent::Speech {
                    audio: Some(audio), ..
                } = &mut broken.clips[0].content
                else {
                    panic!()
                };
                match mutation {
                    0 => audio.sbv2_source = None,
                    1 => {
                        audio
                            .sbv2_source
                            .as_mut()
                            .unwrap()
                            .selection
                            .metadata_sha256 = "f".repeat(64)
                    }
                    _ => audio.sbv2_source.as_mut().unwrap().sample_rate = 48000,
                }
                assert!(
                    !sungeki_speech::audio_reference_matches(&broken, input.clip_id())
                        .unwrap_or(false)
                );
                if mutation == 2 {
                    assert!(sungeki_project::validate(&broken).is_err());
                }
            }
            for case in 0..7 {
                let mut project = baseline.clone();
                match case {
                    0 => project.clips[0].start_us.0 += 123,
                    1 => project.characters[0].subtitle_style.font_size = 72.,
                    2 => {
                        let ClipContent::Speech { subtitle_text, .. } =
                            &mut project.clips[0].content
                        else {
                            panic!()
                        };
                        *subtitle_text = "別のセリフ".into();
                    }
                    3 => {
                        let sungeki_project::Voice::Sbv2 { settings, .. } =
                            &mut project.characters[0].voice
                        else {
                            panic!()
                        };
                        settings.length = 1.5;
                    }
                    4 => {
                        let sungeki_project::Voice::Sbv2 { identity, .. } =
                            &mut project.characters[0].voice
                        else {
                            panic!()
                        };
                        identity.engine_revision = "another".into();
                    }
                    5 => {
                        project
                            .pronunciation_dictionary
                            .push(sungeki_project::PronunciationEntry {
                                written: "字幕".into(),
                                spoken: "じまく".into(),
                            })
                    }
                    _ => {
                        let ClipContent::Speech { audio, .. } = &mut project.clips[0].content
                        else {
                            panic!()
                        };
                        *audio = None;
                    }
                }
                assert_eq!(
                    sungeki_speech::audio_reference_matches(&project, input.clip_id()).unwrap(),
                    case < 2,
                    "offline case {case}"
                );
                shared
                    .lock()
                    .unwrap()
                    .request(SessionRequest::Edit {
                        session_id: sid.clone(),
                        edit: EditRequest {
                            operation_id: id(),
                            expected_revision: 3 + case,
                            action: EditAction::Apply {
                                commands: vec![sungeki_editor::Command::ReplaceProject {
                                    project,
                                    checkpoint_id: None,
                                }],
                            },
                        },
                    })
                    .unwrap();
                assert_eq!(job_status(&shared, &job.id).stale, case >= 2, "case {case}");
            }
        }
    }
}

fn job_status(
    shared: &Arc<Mutex<crate::sessions::Sessions>>,
    id: &str,
) -> sungeki_protocol::SpeechJob {
    let sungeki_protocol::SessionResponse::SpeechJob { job } = crate::speech_api::execute(
        shared,
        sungeki_protocol::SpeechJobRequest::Get {
            job_id: id.into(),
            wait_ms: 0,
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(job.phase, JobPhase::Succeeded);
    assert!(!job.engine_settlement_required);
    assert_eq!(job.artifact.as_ref().unwrap().sample_rate, 44100);
    job
}
