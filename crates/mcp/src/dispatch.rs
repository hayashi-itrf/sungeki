use crate::context::Context;
use rmcp::ErrorData;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sungeki_protocol::*;
pub(crate) fn parse<T: DeserializeOwned>(args: Value) -> Result<T, ErrorData> {
    serde_json::from_value(args).map_err(|e| ErrorData::invalid_params(e.to_string(), None))
}
pub fn request(
    name: &str,
    args: Value,
    context: &mut Context,
) -> Result<(SessionRequest, bool), ErrorData> {
    for dispatch in [
        crate::presets::request,
        crate::collection_dispatch::request,
        crate::build_dispatch::request,
        crate::loudness_dispatch::request,
        crate::psd_dispatch::request,
        crate::frame_preview_dispatch::request,
        crate::audio_preview_dispatch::request,
        crate::render_dispatch::request,
    ] {
        if let Some(request) = dispatch(name, args.clone())? {
            return Ok((request, false));
        }
    }
    let request = match name {
        "parse_markdown_script" => SessionRequest::ParseMarkdownScript { args: parse(args)? },
        "install_speech_config" => SessionRequest::InstallSpeechConfig { args: parse(args)? },
        "get_runtime_settings" => {
            let _: EmptyArgs = parse(args)?;
            SessionRequest::GetRuntimeSettings
        }
        "get_runtime_environment" => {
            let _: EmptyArgs = parse(args)?;
            SessionRequest::GetRuntimeEnvironment {}
        }
        "save_runtime_settings" => SessionRequest::SaveRuntimeSettings { args: parse(args)? },
        "read_review_image" => SessionRequest::ReadReviewImage { args: parse(args)? },
        "export_subtitles" => SessionRequest::ExportSubtitles { args: parse(args)? },
        "analyze_audio" => SessionRequest::AnalyzeAudio { args: parse(args)? },
        "get_sbv2_profile" => SessionRequest::SpeechJob {
            request: SpeechJobRequest::GetSbv2Profile { args: parse(args)? },
        },
        "list_sbv2_profiles" => {
            let _: EmptyArgs = parse(args)?;
            SessionRequest::SpeechJob {
                request: SpeechJobRequest::ListSbv2Profiles,
            }
        }
        "list_qwen_profiles" => {
            let _: EmptyArgs = parse(args)?;
            SessionRequest::SpeechJob {
                request: SpeechJobRequest::ListQwenProfiles,
            }
        }
        "get_qwen_profile" => SessionRequest::SpeechJob {
            request: SpeechJobRequest::GetQwenProfile { args: parse(args)? },
        },
        "start_qwen_speech_job" => SessionRequest::SpeechJob {
            request: SpeechJobRequest::StartQwen { args: parse(args)? },
        },
        "list_irodori_profiles" => {
            let _: EmptyArgs = parse(args)?;
            SessionRequest::SpeechJob {
                request: SpeechJobRequest::ListIrodoriProfiles,
            }
        }
        "get_irodori_profile" => SessionRequest::SpeechJob {
            request: SpeechJobRequest::GetIrodoriProfile { args: parse(args)? },
        },
        "start_irodori_speech_job" => SessionRequest::SpeechJob {
            request: SpeechJobRequest::StartIrodori { args: parse(args)? },
        },
        "start_sbv2_speech_job" => SessionRequest::SpeechJob {
            request: SpeechJobRequest::StartSbv2 { args: parse(args)? },
        },
        "start_speech_job" => SessionRequest::SpeechJob {
            request: SpeechJobRequest::Start { args: parse(args)? },
        },
        "get_speech_job" => {
            let args: GetJobArgs = parse(args)?;
            SessionRequest::SpeechJob {
                request: SpeechJobRequest::Get {
                    job_id: args.job_id,
                    wait_ms: args.wait_ms.unwrap_or(0),
                },
            }
        }
        "cancel_speech_job" => SessionRequest::SpeechJob {
            request: SpeechJobRequest::Cancel {
                job_id: parse::<CancelJobArgs>(args)?.job_id,
            },
        },
        "list_speech_jobs" => {
            let args: ListJobsArgs = parse(args)?;
            SessionRequest::SpeechJob {
                request: SpeechJobRequest::List {
                    session_id: args.session_id,
                    after: args.after,
                    limit: args.limit.unwrap_or(20),
                },
            }
        }
        "get_voicevox_speaker" => SessionRequest::GetVoicevoxSpeaker { args: parse(args)? },
        "get_local_speech_policy" => SessionRequest::GetLocalSpeechPolicy { args: parse(args)? },
        "get_local_speech_catalog" => SessionRequest::GetLocalSpeechCatalog { args: parse(args)? },
        "synthesize_local_speech" => SessionRequest::SpeechJob {
            request: SpeechJobRequest::StartLocal { args: parse(args)? },
        },
        "discover_voicevox" => SessionRequest::DiscoverVoicevox { args: parse(args)? },
        "get_voicevox_catalog" => SessionRequest::GetVoicevoxCatalog { args: parse(args)? },
        "preview_pronunciation" => {
            let args: PreviewPronunciationArgs = parse(args)?;
            let id = context.session(args.session_id)?;
            SessionRequest::PreviewPronunciation {
                expected_revision: context.revision(&id, args.expected_revision)?,
                session_id: id,
                subtitle_text: args.subtitle_text,
                spoken_text: args.spoken_text,
            }
        }
        "review_project" => {
            let args: ReviewProjectArgs = parse(args)?;
            let id = context.session(args.session_id)?;
            let key = (id.clone(), args.job_id.clone());
            let revision = context.revision(
                &id,
                args.expected_revision
                    .or_else(|| context.review_attempted.get(&key).copied()),
            )?;
            context.review_attempted.insert(key, revision);
            SessionRequest::StartReview {
                sampling: args.sampling,
                session_id: id,
                job_id: args.job_id,
                expected_revision: revision,
                wait_ms: args.wait_ms.unwrap_or(0),
            }
        }
        "get_job" => {
            let args: GetJobArgs = parse(args)?;
            SessionRequest::GetReviewJob {
                job_id: args.job_id,
                wait_ms: args.wait_ms.unwrap_or(0),
            }
        }
        "cancel_job" => SessionRequest::CancelReviewJob {
            job_id: parse::<CancelJobArgs>(args)?.job_id,
        },
        "list_jobs" => {
            let args: ListJobsArgs = parse(args)?;
            SessionRequest::ListReviewJobs {
                session_id: args.session_id,
                after: args.after,
                limit: args.limit.unwrap_or(20),
            }
        }

        "probe_image" => SessionRequest::ProbeImage {
            path: parse::<ProbeImageArgs>(args)?.path,
        },
        "create_project" => {
            return Ok((
                SessionRequest::Create {
                    title: parse::<CreateProjectArgs>(args)?.title,
                },
                true,
            ));
        }
        "open_project" => {
            let args = parse::<OpenProjectArgs>(args)?;
            return Ok((
                SessionRequest::Open {
                    path: args.path,
                    import_checkpoints: args.import_checkpoints,
                },
                true,
            ));
        }
        "list_sessions" => {
            parse::<EmptyArgs>(args)?;
            SessionRequest::List {}
        }
        "attach_session" => {
            return Ok((
                SessionRequest::Get {
                    session_id: parse::<AttachSessionArgs>(args)?.session_id,
                },
                true,
            ));
        }
        "delete_checkpoint" => {
            let args: DeleteCheckpointArgs = parse(args)?;
            SessionRequest::Checkpoint {
                session_id: context.session(args.session_id)?,
                request: CheckpointRequest::Delete {
                    checkpoint_id: args.checkpoint_id,
                },
            }
        }
        "compare_checkpoints" => {
            let args: CompareCheckpointArgs = parse(args)?;
            SessionRequest::Checkpoint {
                session_id: context.session(args.session_id)?,
                request: CheckpointRequest::Compare {
                    checkpoint_id: args.checkpoint_id,
                    target: args.target,
                    offset: args.offset.unwrap_or(0),
                    limit: args.limit.unwrap_or(20),
                },
            }
        }
        "create_checkpoint" => {
            let args: CreateCheckpointArgs = parse(args)?;
            SessionRequest::Checkpoint {
                session_id: context.session(args.session_id)?,
                request: CheckpointRequest::Create {
                    checkpoint_id: args.checkpoint_id,
                    name: args.name,
                    expected_revision: args.expected_revision,
                },
            }
        }
        "list_checkpoints" => {
            let args: ListCheckpointsArgs = parse(args)?;
            SessionRequest::Checkpoint {
                session_id: context.session(args.session_id)?,
                request: CheckpointRequest::List {
                    after: args.after,
                    limit: args.limit.unwrap_or(20),
                },
            }
        }
        "restore_checkpoint" => {
            let args: RestoreCheckpointArgs = parse(args)?;
            SessionRequest::Checkpoint {
                session_id: context.session(args.session_id)?,
                request: CheckpointRequest::Restore {
                    checkpoint_id: args.checkpoint_id,
                    operation_id: args.operation_id,
                    expected_revision: args.expected_revision,
                },
            }
        }
        "get_project" => {
            let args: GetProjectArgs = parse(args)?;
            let id = context.session(args.session_id)?;
            if args.view.is_none()
                && args.expected_revision.is_none()
                && args.scene_ids.is_none()
                && args.clip_ids.is_none()
                && args.offset.is_none()
                && args.limit.is_none()
            {
                SessionRequest::Get { session_id: id }
            } else {
                let view = args.view.unwrap_or(ProjectView::Full);
                if matches!(view, ProjectView::Full | ProjectView::Summary)
                    && (args.scene_ids.is_some()
                        || args.clip_ids.is_some()
                        || args.offset.is_some()
                        || args.limit.is_some())
                {
                    return Err(ErrorData::invalid_params(
                        "絞り込みとページ指定にはscenesまたはtimelineを選択してください",
                        None,
                    ));
                }
                SessionRequest::ReadProject {
                    session_id: id,
                    expected_revision: args.expected_revision,
                    query: ProjectQuery {
                        view,
                        scene_ids: args.scene_ids,
                        clip_ids: args.clip_ids,
                        offset: args.offset.unwrap_or(0),
                        limit: args.limit.unwrap_or(20),
                    },
                }
            }
        }
        "apply_commands" => {
            let args: ApplyCommandsArgs = parse(args)?;
            let id = context.session(args.session_id)?;
            if args.dry_run.unwrap_or(false) {
                SessionRequest::PreviewEdit {
                    expected_revision: context.revision(&id, args.expected_revision)?,
                    session_id: id,
                    commands: args.commands,
                    offset: args.offset.unwrap_or(0),
                    limit: args.limit.unwrap_or(20),
                }
            } else {
                if args.offset.is_some() || args.limit.is_some() {
                    return Err(ErrorData::invalid_params(
                        "offset/limitはdry_run専用です",
                        None,
                    ));
                }
                let operation_id = args.operation_id.ok_or_else(|| {
                    ErrorData::invalid_params("確定編集にはoperation_idが必要です", None)
                })?;
                SessionRequest::Edit {
                    edit: EditRequest {
                        expected_revision: context.operation_revision(
                            &id,
                            &operation_id,
                            args.expected_revision,
                        )?,
                        operation_id,
                        action: EditAction::Apply {
                            commands: args.commands,
                        },
                    },
                    session_id: id,
                }
            }
        }
        "save_project" => {
            let args: SaveProjectArgs = parse(args)?;
            let id = context.session(args.session_id)?;
            SessionRequest::Save {
                expected_revision: context.revision(&id, args.expected_revision)?,
                session_id: id,
                path: args.path,
            }
        }
        "undo" | "redo" => {
            let args: HistoryArgs = parse(args)?;
            let id = context.session(args.session_id)?;
            let action = if name == "undo" {
                EditAction::Undo {
                    expected_operation_id: args.expected_operation_id,
                }
            } else {
                EditAction::Redo {
                    expected_operation_id: args.expected_operation_id,
                }
            };
            SessionRequest::Edit {
                edit: EditRequest {
                    operation_id: args.operation_id.clone(),
                    expected_revision: context.operation_revision(
                        &id,
                        &args.operation_id,
                        args.expected_revision,
                    )?,
                    action,
                },
                session_id: id,
            }
        }
        _ => return Err(ErrorData::invalid_params("未知のツールです", None)),
    };
    if let SessionRequest::Edit { session_id, edit } = &request {
        context.attempted.insert(
            (session_id.clone(), edit.operation_id.clone()),
            edit.expected_revision,
        );
    }
    Ok((request, false))
}
