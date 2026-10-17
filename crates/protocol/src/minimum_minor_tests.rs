use super::*;
use sungeki_editor::{Command, EditAction, EditRequest};
#[test]
fn psd_preview_requires_host_with_time_evaluation() {
    let request = SessionRequest::PsdPreview {
        session_id: "session".into(),
        expected_revision: 1,
        clip_id: "standing".into(),
        time_us: sungeki_project::TimeUs(300_000),
    };
    assert_eq!(request.minimum_protocol_minor(), 34);
}
#[test]
fn metadata_edits_and_dry_runs_require_the_new_host() {
    let commands = vec![Command::SetAssetMetadata {
        asset_id: "asset".into(),
        name: "素材".into(),
        source: None,
        credit: "作者".into(),
    }];
    let mut preview = SessionRequest::PreviewEdit {
        session_id: "session".into(),
        expected_revision: 1,
        commands: commands.clone(),
        offset: 0,
        limit: 10,
    };
    let mut edit = SessionRequest::Edit {
        session_id: "session".into(),
        edit: EditRequest {
            operation_id: "operation".into(),
            expected_revision: 1,
            action: EditAction::Apply { commands },
        },
    };
    assert_eq!(preview.minimum_protocol_minor(), 13);
    assert_eq!(edit.minimum_protocol_minor(), 13);
    if let SessionRequest::PreviewEdit { commands, .. } = &mut preview {
        commands.push(Command::ResizeClip {
            clip_id: "clip".into(),
            duration_us: sungeki_project::TimeUs(1),
        });
    }
    if let SessionRequest::Edit { edit, .. } = &mut edit {
        edit.action = EditAction::Apply {
            commands: vec![Command::ResizeClip {
                clip_id: "clip".into(),
                duration_us: sungeki_project::TimeUs(1),
            }],
        };
    }
    assert_eq!(preview.minimum_protocol_minor(), 14);
    assert_eq!(edit.minimum_protocol_minor(), 14);
    let duplicate = Command::DuplicateClip {
        clip_id: "clip".into(),
        new_clip_id: "copy".into(),
        start_us: sungeki_project::TimeUs(0),
    };
    if let SessionRequest::PreviewEdit { commands, .. } = &mut preview {
        commands.push(duplicate.clone());
    }
    if let SessionRequest::Edit { edit, .. } = &mut edit {
        edit.action = EditAction::Apply {
            commands: vec![duplicate],
        };
    }
    assert_eq!(preview.minimum_protocol_minor(), 15);
    assert_eq!(edit.minimum_protocol_minor(), 15);
    for command in [
        Command::SetTrackProperties {
            track_id: "track".into(),
            name: "name".into(),
            visible: true,
            muted: false,
        },
        Command::ReorderTracks { track_ids: vec![] },
        Command::SetClipTrack {
            clip_id: "clip".into(),
            track_id: "track".into(),
        },
    ] {
        if let SessionRequest::PreviewEdit { commands, .. } = &mut preview {
            *commands = vec![command.clone()];
        }
        if let SessionRequest::Edit { edit, .. } = &mut edit {
            edit.action = EditAction::Apply {
                commands: vec![command],
            };
        }
        assert_eq!(preview.minimum_protocol_minor(), 16);
        assert_eq!(edit.minimum_protocol_minor(), 16);
    }
    for command in [
        Command::SetTextStyle {
            clip_id: "clip".into(),
            font_size: 48.,
            color: "#123456".into(),
        },
        Command::SetShape {
            clip_id: "clip".into(),
            shape: sungeki_project::ShapeKind::Ellipse,
            fill: "#123456".into(),
        },
    ] {
        if let SessionRequest::PreviewEdit { commands, .. } = &mut preview {
            *commands = vec![command.clone()];
        }
        if let SessionRequest::Edit { edit, .. } = &mut edit {
            edit.action = EditAction::Apply {
                commands: vec![command],
            };
        }
        assert_eq!(preview.minimum_protocol_minor(), 17);
        assert_eq!(edit.minimum_protocol_minor(), 17);
    }
    let canvas = sungeki_project::Project::empty("project".into(), "title".into()).canvas;
    if let SessionRequest::PreviewEdit { commands, .. } = &mut preview {
        commands.push(Command::SetCanvas {
            canvas: canvas.clone(),
        });
    }
    if let SessionRequest::Edit { edit, .. } = &mut edit {
        edit.action = EditAction::Apply {
            commands: vec![Command::SetCanvas { canvas }],
        };
    }
    assert_eq!(preview.minimum_protocol_minor(), 18);
    assert_eq!(edit.minimum_protocol_minor(), 18);
    let dictionary = Command::SetPronunciationDictionary { entries: vec![] };
    if let SessionRequest::PreviewEdit { commands, .. } = &mut preview {
        commands.push(dictionary.clone());
    }
    if let SessionRequest::Edit { edit, .. } = &mut edit {
        edit.action = EditAction::Apply {
            commands: vec![dictionary],
        };
    }
    assert_eq!(preview.minimum_protocol_minor(), 27);
    assert_eq!(edit.minimum_protocol_minor(), 27);
    assert_eq!(
        SessionRequest::PreviewPronunciation {
            session_id: "s".into(),
            expected_revision: 0,
            subtitle_text: "字幕".into(),
            spoken_text: None,
        }
        .minimum_protocol_minor(),
        21
    );
    for request in [
        SessionRequest::StartReview {
            sampling: None,
            session_id: "s".into(),
            job_id: "j".into(),
            expected_revision: 0,
            wait_ms: 0,
        },
        SessionRequest::GetReviewJob {
            job_id: "j".into(),
            wait_ms: 0,
        },
        SessionRequest::CancelReviewJob { job_id: "j".into() },
        SessionRequest::ListReviewJobs {
            session_id: None,
            after: None,
            limit: 10,
        },
    ] {
        assert_eq!(request.minimum_protocol_minor(), 73);
    }
    assert_eq!(SessionRequest::List {}.minimum_protocol_minor(), 11);
}

#[test]
fn publication_requires_host_with_verified_copy() {
    let request = SessionRequest::RenderJob {
        request: RenderJobRequest::Publish {
            args: PublishRenderArgs {
                job_id: "job".into(),
                path: "/tmp/movie.mp4".into(),
            },
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 35);
}

#[test]
fn waveform_analysis_requires_minor_36() {
    let request = SessionRequest::AnalyzeAudio {
        args: AnalyzeAudioArgs {
            session_id: "s".into(),
            expected_revision: 0,
            asset_id: "a".into(),
            base_frames: 256,
            level: 0,
            channel: 0,
            start_bucket: 0,
            count: 100,
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 36);
}

#[test]
fn gain_apply_preview_and_embedded_project_require_minor_37() {
    use sungeki_editor::Command;
    use sungeki_project::{ClipContent, Project};
    let mut p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let speech = p
        .clips
        .iter_mut()
        .find(|c| matches!(c.content, ClipContent::Speech { .. }))
        .unwrap();
    let ClipContent::Speech { gain_db, .. } = &mut speech.content else {
        unreachable!()
    };
    *gain_db = Some(-6.0);
    let clip = speech.clone();
    for command in [
        Command::SetSpeechGain {
            clip_id: clip.id.clone(),
            gain_db: None,
        },
        Command::AddClip { clip },
        Command::ReplaceProject {
            checkpoint_id: None,
            project: p,
        },
    ] {
        assert_eq!(
            SessionRequest::PreviewEdit {
                session_id: "s".into(),
                expected_revision: 0,
                commands: vec![command.clone()],
                offset: 0,
                limit: 10
            }
            .minimum_protocol_minor(),
            37
        );
        assert_eq!(
            SessionRequest::Edit {
                session_id: "s".into(),
                edit: EditRequest {
                    operation_id: "o".into(),
                    expected_revision: 0,
                    action: EditAction::Apply {
                        commands: vec![command]
                    }
                }
            }
            .minimum_protocol_minor(),
            37
        );
    }
}

#[test]
fn loudness_jobs_require_minor_38() {
    use crate::{LoudnessJobRequest, StartLoudnessArgs};
    for request in [
        LoudnessJobRequest::Start {
            args: StartLoudnessArgs {
                wait_ms: None,
                session_id: "s".into(),
                job_id: "j".into(),
                expected_revision: 0,
                asset_id: "a".into(),
            },
        },
        LoudnessJobRequest::Cancel { job_id: "j".into() },
        LoudnessJobRequest::List {
            session_id: None,
            after: None,
            limit: 20,
        },
    ] {
        assert_eq!(
            crate::SessionRequest::LoudnessJob { request }.minimum_protocol_minor(),
            38
        );
    }
}

#[test]
fn subtitle_export_requires_minor_63_and_rejects_unknown_options() {
    let args = serde_json::json!({"session_id":"session", "expected_revision":1,
        "path":"/tmp/subtitles.srt", "format":"srt"});
    let request = SessionRequest::ExportSubtitles {
        args: serde_json::from_value(args.clone()).unwrap(),
    };
    assert_eq!(request.minimum_protocol_minor(), 63);
    let mut unknown = args;
    unknown["overwrite"] = serde_json::json!(true);
    assert!(serde_json::from_value::<ExportSubtitlesArgs>(unknown).is_err());
}

#[test]
fn narration_start_and_get_require_representative_report_support() {
    let mut recipe: sungeki_project::BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe.preset_id = "narration-wide-v1".into();
    let request = SessionRequest::BuildVideo {
        request: BuildVideoRequest::Start {
            args: StartBuildVideoArgs {
                wait_ms: None,
                reuse_from: None,
                managed_profiles: Default::default(),
                job_id: "job".into(),
                recipe,
                project_path: "/tmp/new.vvm.json".into(),
                base_url: "http://127.0.0.1:50021".into(),
                engine_version: "test".into(),
            },
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 73);
    let request = SessionRequest::BuildVideo {
        request: BuildVideoRequest::Get {
            job_id: "job".into(),
            wait_ms: 0,
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 135);
}

#[test]
fn sbv2_start_requires_profile_aware_host() {
    let request = SessionRequest::SpeechJob {
        request: SpeechJobRequest::StartSbv2 {
            args: StartSbv2SpeechArgs {
                wait_ms: None,
                session_id: "session".into(),
                job_id: "job".into(),
                expected_revision: 1,
                clip_id: "clip".into(),
                profile_id: "local-jp".into(),
            },
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 68);
    let mut encoded = serde_json::to_value(&request).unwrap();
    encoded["request"]["args"]["command"] = serde_json::json!("arbitrary command");
    assert!(serde_json::from_value::<SessionRequest>(encoded).is_err());
}

#[test]
fn sbv2_profile_listing_requires_minor69_without_raising_start_requirement() {
    let request = SessionRequest::SpeechJob {
        request: SpeechJobRequest::ListSbv2Profiles,
    };
    assert_eq!(request.minimum_protocol_minor(), 69);
}

#[test]
fn inspected_sbv2_details_require_minor70() {
    let request = SessionRequest::SpeechJob {
        request: SpeechJobRequest::GetSbv2Profile {
            args: GetSbv2ProfileArgs {
                profile_id: "local-jp".into(),
            },
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 70);
}

#[test]
fn frame_preview_requires_minor71() {
    let request = crate::SessionRequest::FramePreview {
        request: crate::FramePreviewRequest::Get {
            job_id: "fixture".into(),
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 71);
}
