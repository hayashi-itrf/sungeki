use crate::sessions::Sessions;
use std::sync::{Arc, Mutex};
use sungeki_protocol::*;
#[path = "editing_audio.rs"]
mod editing_audio;
#[path = "preset_instantiate.rs"]
mod preset_instantiate;
#[path = "preset_api.rs"]
mod presets;
#[path = "vrm_asset_read.rs"]
mod vrm_asset_read;

impl Sessions {
    pub fn execute(
        shared: &Arc<Mutex<Self>>,
        request: SessionRequest,
    ) -> Result<SessionResponse, ApiError> {
        if let SessionRequest::ReadVrmAsset {
            session_id,
            expected_revision,
            asset_id,
            offset,
            length,
        } = request
        {
            return vrm_asset_read::read(
                shared,
                session_id,
                expected_revision,
                asset_id,
                offset,
                length,
            );
        }
        if let SessionRequest::ReadEditingAudio {
            session_id,
            expected_revision,
            clip_id,
            offset,
            length,
        } = request
        {
            return editing_audio::read(
                shared,
                session_id,
                expected_revision,
                clip_id,
                offset,
                length,
            );
        }
        if let SessionRequest::ParseMarkdownScript { args } = request {
            let scenes = sungeki_editor::parse_markdown_script(&args.source, &args.roles).map_err(
                |cause| {
                    let mut error = ApiError::new(ErrorCode::ValidationFailed, cause.to_string());
                    error.details.insert("line".into(), cause.line.to_string());
                    error
                },
            )?;
            return Ok(SessionResponse::MarkdownScript { scenes });
        }
        if let SessionRequest::InstantiatePreset { args } = request {
            return preset_instantiate::execute(shared, args);
        }
        if let SessionRequest::Presets { request } = request {
            return presets::execute(shared, request);
        }
        if let SessionRequest::InstallSpeechConfig { args } = request {
            let (database, cancel) = crate::speech_api::profile_inspection(shared)?;
            let directory = database.parent().ok_or_else(|| {
                ApiError::new(ErrorCode::ValidationFailed, "設定フォルダーがありません")
            })?;
            let configuration_path = crate::startup::speech_config_install::install(
                directory,
                args.engine.as_str(),
                &args.candidate_path,
                &cancel,
            )?;
            return Ok(SessionResponse::SpeechConfigInstalled { configuration_path });
        }
        if let SessionRequest::GetRuntimeSettings = request {
            return crate::runtime_settings::execute(shared, None);
        }
        if let SessionRequest::GetRuntimeEnvironment {} = request {
            return crate::runtime_settings::environment::execute(shared);
        }
        if let SessionRequest::SaveRuntimeSettings { args } = request {
            return crate::runtime_settings::execute(shared, Some(args));
        }
        if let SessionRequest::Proxy { request } = request {
            return crate::image_probe::video::proxy_api::execute(shared, request);
        }
        if let SessionRequest::VideoPreview { args } = request {
            return crate::image_probe::video_preview::execute(shared, args);
        }
        if let SessionRequest::Collection { request } = request {
            return crate::collection_execute::api::execute(shared, request);
        }
        if let SessionRequest::ReadReviewImage { args } = request {
            return crate::review_worker::image_read::execute(shared, args);
        }
        if let SessionRequest::ExportSubtitles { args } = request {
            return crate::subtitle_export::execute(shared, args);
        }
        if let SessionRequest::BuildVideo { request } = request {
            return crate::build_record::execute(shared, request);
        }
        if let SessionRequest::FramePreview { request } = request {
            return crate::frame_preview_api::execute(shared, request);
        }
        if let SessionRequest::AudioPreview { request } = request {
            return crate::audio_preview_api::execute(shared, request);
        }
        if let SessionRequest::ProposeSpeechGain { args } = request {
            return crate::gain_proposal::execute(shared, args);
        }
        if let SessionRequest::LoudnessJob { request } = request {
            return crate::loudness_api::execute(shared, request);
        }
        if let SessionRequest::AnalyzeAudio { args } = request {
            return crate::audio_analysis::execute(shared, args);
        }
        if let SessionRequest::PsdPreview {
            session_id,
            expected_revision,
            clip_id,
            time_us,
        } = request
        {
            return crate::psd_preview::execute(
                shared,
                session_id,
                expected_revision,
                clip_id,
                time_us,
            );
        }
        if let SessionRequest::ProbePsd { path } = request {
            let database = shared
                .lock()
                .map_err(crate::render_record::invalid)?
                .review_database_path
                .clone();
            return crate::psd_probe::execute(&database, path);
        }
        if let SessionRequest::RenderJob { request } = request {
            return crate::render_api::execute(shared, request);
        }
        if let SessionRequest::SpeechJob { request } = request {
            return crate::speech_api::execute(shared, request);
        }
        if let SessionRequest::PreviewPronunciation {
            session_id,
            expected_revision,
            subtitle_text,
            spoken_text,
        } = request
        {
            return crate::pronunciation::preview(
                shared,
                session_id,
                expected_revision,
                subtitle_text,
                spoken_text,
            );
        }
        match &request {
            SessionRequest::StartReview {
                sampling,
                session_id,
                job_id,
                expected_revision,
                wait_ms,
            } => {
                if *wait_ms > 30000 {
                    return Err(ApiError::new(
                        ErrorCode::ValidationFailed,
                        "待機は0〜30000msです",
                    ));
                }
                let started = crate::review_jobs::start(
                    shared,
                    session_id.clone(),
                    job_id.clone(),
                    *expected_revision,
                    sampling.clone().unwrap_or_default(),
                )?;
                return if *wait_ms == 0 {
                    Ok(started)
                } else {
                    crate::review_jobs::get(shared, job_id.clone(), *wait_ms)
                };
            }
            SessionRequest::GetReviewJob { job_id, wait_ms } => {
                return crate::review_jobs::get(shared, job_id.clone(), *wait_ms);
            }
            SessionRequest::CancelReviewJob { job_id } => {
                return crate::review_jobs::cancel(shared, job_id.clone());
            }
            SessionRequest::ListReviewJobs {
                session_id,
                after,
                limit,
            } => {
                if !(1..=100).contains(limit) {
                    return Err(ApiError::new(
                        ErrorCode::ValidationFailed,
                        "件数は1〜100です",
                    ));
                }
                let sessions = shared.lock().map_err(crate::review_record::invalid)?;
                let job_ids = sessions
                    .store
                    .list_jobs(
                        Some(sungeki_storage::JobKind::Review),
                        session_id.as_deref(),
                        after.as_deref(),
                        *limit,
                    )
                    .map_err(crate::sessions::storage_error)?;
                return Ok(SessionResponse::ReviewJobs { job_ids });
            }
            _ => {}
        }
        if let SessionRequest::ImagePreview {
            session_id,
            asset_id,
            expected_revision,
        } = request
        {
            let asset = {
                let sessions = shared
                    .lock()
                    .map_err(|_| ApiError::new(ErrorCode::IoError, "ロック障害"))?;
                let (_, state) = sessions.load(&session_id)?;
                if state.history().revision() != expected_revision {
                    return Err(ApiError::new(ErrorCode::Conflict, "作品が更新されています"));
                }
                state
                    .history()
                    .project()
                    .assets
                    .iter()
                    .find(|a| a.id == asset_id)
                    .cloned()
                    .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "素材が見つかりません"))?
            };
            return crate::image_probe::preview(asset);
        }
        if let SessionRequest::ProbeVrm { path, kind } = request {
            return crate::image_probe::video::vrm_import::execute(shared, path, kind);
        }
        if let SessionRequest::ProbeVideo { path } = request {
            return crate::image_probe::video::execute(shared, path);
        }
        if let SessionRequest::ProbeAudio { path } = request {
            return crate::audio_probe::execute(shared, path);
        }
        if let SessionRequest::ProbeImage { path } = request {
            return crate::image_probe::execute(path);
        }
        if let SessionRequest::Open {
            path,
            import_checkpoints,
        } = request
        {
            return crate::session_open::open(shared, path, import_checkpoints);
        }
        if let SessionRequest::Save {
            session_id,
            path,
            expected_revision,
        } = request
        {
            crate::session_save::save(shared, session_id, path, expected_revision)
        } else {
            shared
                .lock()
                .map_err(|_| ApiError::new(ErrorCode::IoError, "ロック障害"))?
                .request(request)
        }
    }
}
