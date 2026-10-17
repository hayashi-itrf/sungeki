mod cache;
mod edit;
use crate::session_save::{SavePlan, document_error};
use std::{collections::HashSet, path::Path};
use sungeki_editor::EditSession;
use sungeki_project::Project;
use sungeki_protocol::*;
use sungeki_storage::{RecoveryStore, StoredSession};

pub struct Sessions {
    /// Cancels read-only model inspection when this host shuts down.
    pub(crate) profile_inspection_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(crate) build_runtime: crate::build_record::Runtime,
    pub(crate) loudness_runtime: crate::media_job_runtime::Runtime,
    pub(crate) render_runtime: crate::render_runtime::Runtime,
    pub(crate) speech_runtime: crate::speech_runtime::Runtime,
    pub(crate) speech_directory: Result<std::path::PathBuf, ApiError>,
    pub(crate) speech_resources: sungeki_jobs::Resources,
    pub(crate) review_database_path: std::path::PathBuf,
    pub(crate) review_directory: Result<std::path::PathBuf, ApiError>,
    pub(crate) review_runtime: crate::review_jobs::Runtime,
    pub(crate) events: crate::events::Events,
    pub(crate) store: RecoveryStore,
    saving: HashSet<String>,
    pub(crate) parsed_cache: cache::ParsedCache,
}
pub(crate) use crate::session_errors::{edit_error, preflight, storage_error};

impl Sessions {
    pub(crate) fn snapshot(
        &self,
        id: String,
        session: &EditSession,
    ) -> Result<SessionSnapshot, ApiError> {
        let binding = self.store.document_binding(&id).map_err(storage_error)?;
        let hash =
            sungeki_storage::project_hash(session.history().project()).map_err(document_error)?;
        let h = session.history();
        Ok(SessionSnapshot {
            event_cursor: self.events.cursor(),
            document_path: binding.as_ref().map(|b| b.path.clone()),
            dirty: binding
                .as_ref()
                .is_none_or(|b| b.saved_content_hash != hash),
            session_id: id,
            project: h.project().clone(),
            revision: h.revision(),
            undo_operation_id: h.undo_operation_id().map(str::to_owned),
            redo_operation_id: h.redo_operation_id().map(str::to_owned),
        })
    }

    pub fn open(path: &Path) -> Result<Self, ApiError> {
        let store = RecoveryStore::open(path).map_err(storage_error)?;
        let mut result = Self {
            profile_inspection_cancel: Default::default(),
            build_runtime: Default::default(),
            loudness_runtime: Default::default(),
            render_runtime: Default::default(),
            speech_runtime: Default::default(),
            speech_directory: if cfg!(test) {
                crate::speech_artifacts::root(path)
            } else {
                Err(ApiError::new(
                    ErrorCode::CapabilityUnavailable,
                    "音声保存先は合成開始時に確認します",
                ))
            },
            speech_resources: sungeki_jobs::Resources::default(),
            review_database_path: path.to_path_buf(),
            review_directory: crate::review_workdir::root(path),
            review_runtime: crate::review_jobs::Runtime::default(),
            events: crate::events::Events::new(),
            store,
            saving: HashSet::new(),
            parsed_cache: Default::default(),
        };
        crate::session_save::recover_pending(&mut result)?;
        crate::review_jobs::recover(&mut result)?;
        crate::render_record::recover(&mut result)?;
        crate::frame_preview_record::recover(&mut result)?;
        crate::audio_preview_record::recover(&mut result)?;
        crate::image_probe::video::proxy_record::recover(&mut result)?;
        crate::loudness_record::recover(&mut result)?;
        result.speech_resources = crate::speech_journal::recover(&mut result)?;
        crate::build_record::recover(&mut result)?;
        Ok(result)
    }
    pub(crate) fn load(&self, id: &str) -> Result<(StoredSession, EditSession), ApiError> {
        let stored = self
            .store
            .load(id)
            .map_err(storage_error)?
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "セッションが見つかりません"))?;
        let session = match self.parsed_cache.get(&stored) {
            Some(session) => session,
            None => EditSession::restore(&stored.snapshot).map_err(edit_error)?,
        };
        if session.history().revision() != stored.revision {
            return Err(ApiError::new(
                ErrorCode::IoError,
                "復旧DBのrevisionが一致しません",
            ));
        }
        Ok((stored, session))
    }
    pub(crate) fn prepare_save(
        &mut self,
        id: String,
        path: Option<String>,
        revision: u32,
    ) -> Result<SavePlan, ApiError> {
        let (_, session) = self.load(&id)?;
        if revision != session.history().revision() {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "保存対象が更新されています",
            ));
        }
        let binding = self.store.document_binding(&id).map_err(storage_error)?;
        let path = path
            .or_else(|| binding.as_ref().map(|b| b.path.clone()))
            .ok_or_else(|| ApiError::new(ErrorCode::ValidationFailed, "保存先が必要です"))?;
        let path = sungeki_storage::document_location(Path::new(&path)).map_err(document_error)?;
        let key = path.to_string_lossy().into_owned();
        if self.saving.contains(&id) || self.saving.contains(&key) {
            return Err(ApiError::new(ErrorCode::Conflict, "保存中です"));
        }
        if self
            .store
            .document_owner(&key, None)
            .map_err(storage_error)?
            .is_some_and(|owner| owner != id)
        {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "別の文書が保存先を使用しています",
            ));
        }
        let expected = binding.filter(|b| b.path == key).map(|b| b.stamp);
        let content_hash =
            sungeki_storage::project_hash(session.history().project()).map_err(document_error)?;
        let mut preview = self.snapshot(id.clone(), &session)?;
        preview.document_path = Some(key.clone());
        preview.dirty = false;
        preflight(&SessionResponse::Snapshot { snapshot: preview })?;
        self.store
            .begin_save(&sungeki_storage::SaveIntent {
                session_id: id.clone(),
                path: key.clone(),
                content_hash: content_hash.clone(),
            })
            .map_err(storage_error)?;
        self.saving.insert(id.clone());
        self.saving.insert(key);
        Ok(SavePlan {
            session_id: id,
            path,
            project: session.history().project().clone(),
            expected,
            content_hash,
        })
    }
    pub(crate) fn release_save(&mut self, plan: &SavePlan) {
        self.saving.remove(&plan.session_id);
        self.saving.remove(plan.path.to_string_lossy().as_ref());
    }
    pub(crate) fn finish_save(
        &mut self,
        binding: sungeki_storage::DocumentBinding,
    ) -> Result<SessionResponse, ApiError> {
        self.store.bind_document(&binding).map_err(storage_error)?;
        let (_, session) = self.load(&binding.session_id)?;
        self.events.publish(
            &binding.session_id,
            session.history().revision(),
            SessionEventKind::Saved,
            None,
        );
        Ok(SessionResponse::Snapshot {
            snapshot: self.snapshot(binding.session_id, &session)?,
        })
    }
    pub fn request(&mut self, request: SessionRequest) -> Result<SessionResponse, ApiError> {
        let result = self.request_inner(request)?;
        preflight(&result)?;
        Ok(result)
    }
    fn request_inner(&mut self, request: SessionRequest) -> Result<SessionResponse, ApiError> {
        match request {
            SessionRequest::ReadProject {
                session_id,
                expected_revision,
                query,
            } => self.read_project(session_id, expected_revision, query),
            SessionRequest::PreviewEdit {
                session_id,
                expected_revision,
                commands,
                offset,
                limit,
            } => self.preview_edit(session_id, expected_revision, commands, offset, limit),
            SessionRequest::Checkpoint {
                session_id,
                request,
            } => self.checkpoint_request(session_id, request),
            SessionRequest::InstallSpeechConfig { .. }
            | SessionRequest::ParseMarkdownScript { .. }
            | SessionRequest::Presets { .. }
            | SessionRequest::InstantiatePreset { .. }
            | SessionRequest::GetRuntimeSettings
            | SessionRequest::SaveRuntimeSettings { .. }
            | SessionRequest::ExportSubtitles { .. }
            | SessionRequest::VideoPreview { .. }
            | SessionRequest::ProbeVideo { .. }
            | SessionRequest::ProbeVrm { .. }
            | SessionRequest::ProbeAudio { .. }
            | SessionRequest::AnalyzeAudio { .. }
            | SessionRequest::PsdPreview { .. }
            | SessionRequest::ProbePsd { .. }
            | SessionRequest::BuildVideo { .. }
            | SessionRequest::RenderJob { .. }
            | SessionRequest::ReadReviewImage { .. }
            | SessionRequest::FramePreview { .. }
            | SessionRequest::Collection { .. }
            | SessionRequest::AudioPreview { .. }
            | SessionRequest::Proxy { .. }
            | SessionRequest::LoudnessJob { .. }
            | SessionRequest::ProposeSpeechGain { .. }
            | SessionRequest::SpeechJob { .. }
            | SessionRequest::GetVoicevoxSpeaker { .. }
            | SessionRequest::DiscoverVoicevox { .. }
            | SessionRequest::GetVoicevoxCatalog { .. }
            | SessionRequest::StartReview { .. }
            | SessionRequest::GetReviewJob { .. }
            | SessionRequest::CancelReviewJob { .. }
            | SessionRequest::ListReviewJobs { .. }
            | SessionRequest::PreviewPronunciation { .. }
            | SessionRequest::ReadVrmAsset { .. }
            | SessionRequest::ReadEditingAudio { .. }
            | SessionRequest::ImagePreview { .. }
            | SessionRequest::ProbeImage { .. }
            | SessionRequest::Open { .. }
            | SessionRequest::Save { .. } => Err(ApiError::new(
                ErrorCode::ValidationFailed,
                "保存はI/O経路が必要です",
            )),
            SessionRequest::Create { title } => {
                let id = uuid::Uuid::new_v4().to_string();
                let session =
                    EditSession::new(Project::empty(uuid::Uuid::new_v4().to_string(), title))
                        .map_err(|e| ApiError::new(ErrorCode::ValidationFailed, e.to_string()))?;
                preflight(&SessionResponse::Snapshot {
                    snapshot: self.snapshot(id.clone(), &session)?,
                })?;
                self.store
                    .commit(
                        None,
                        &StoredSession {
                            id: id.clone(),
                            revision: 0,
                            version: 0,
                            snapshot: session.snapshot().map_err(edit_error)?.into(),
                        },
                    )
                    .map_err(storage_error)?;
                self.events.publish(&id, 0, SessionEventKind::Created, None);
                Ok(SessionResponse::Snapshot {
                    snapshot: self.snapshot(id, &session)?,
                })
            }
            SessionRequest::List {} => Ok(SessionResponse::Sessions {
                session_ids: self.store.list().map_err(storage_error)?,
            }),
            SessionRequest::Get { session_id } => {
                let (_, session) = self.load(&session_id)?;
                Ok(SessionResponse::Snapshot {
                    snapshot: self.snapshot(session_id, &session)?,
                })
            }
            SessionRequest::Edit { session_id, edit } => self.edit_session(session_id, edit),
        }
    }
}
