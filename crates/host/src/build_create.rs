//! New-only creation. No engine/render dispatch occurs before the plan is durable.
use crate::{
    build_record::decode,
    build_state::{Input, State, invalid},
    sessions::{Sessions, edit_error, preflight, storage_error},
};
use std::path::Path;
use sungeki_editor::EditSession;
use sungeki_project::{BuildRecipe, JobPhase, Project};
use sungeki_protocol::{ApiError, SessionEventKind, SessionResponse};
use sungeki_storage::{JobKind, StoredJob, StoredSession};

pub(crate) fn create(
    sessions: &mut Sessions,
    id: &str,
    recipe: BuildRecipe,
    path: &Path,
) -> Result<StoredJob, ApiError> {
    create_bound(sessions, id, recipe, path, None)
}
pub(crate) fn create_bound(
    sessions: &mut Sessions,
    id: &str,
    recipe: BuildRecipe,
    path: &Path,
    engine: Option<super::SpeechConnection>,
) -> Result<StoredJob, ApiError> {
    create_configured(sessions, id, recipe, path, engine, Default::default())
}
pub(crate) fn create_configured(
    sessions: &mut Sessions,
    id: &str,
    recipe: BuildRecipe,
    path: &Path,
    engine: Option<super::SpeechConnection>,
    managed_profiles: super::profiles::Bindings,
) -> Result<StoredJob, ApiError> {
    create_continued(sessions, id, recipe, path, engine, managed_profiles, None)
}
pub(crate) fn create_continued(
    sessions: &mut Sessions,
    id: &str,
    recipe: BuildRecipe,
    path: &Path,
    engine: Option<super::SpeechConnection>,
    managed_profiles: super::profiles::Bindings,
    continuation: Option<super::continuation::Continuation>,
) -> Result<StoredJob, ApiError> {
    super::profiles::validate(&managed_profiles, &recipe)?;
    let engine = engine.map(|e| e.canonical(&recipe)).transpose()?;
    if uuid::Uuid::parse_str(id)
        .ok()
        .is_none_or(|u| u.to_string() != id)
    {
        return Err(invalid("制作IDには標準形式のUUIDが必要です"));
    }
    recipe.validate().map_err(invalid)?;
    let path = sungeki_storage::document_location(path).map_err(invalid)?;
    if let Some(existing) = sessions.store.load_job(id).map_err(storage_error)? {
        let (input, _) = decode(&existing)?;
        if input.continuation != continuation
            || input.managed_profiles != managed_profiles
            || input.engine != engine
            || input.document_path != path
            || serde_json::to_value(&input.recipe).map_err(invalid)?
                != serde_json::to_value(&recipe).map_err(invalid)?
        {
            return Err(invalid("同じ制作IDが別の入力に使われています"));
        }
        let (_, current) = sessions.load(&existing.session_id)?;
        if current.history().project().id != input.plan.project_id {
            return Err(invalid("制作先の作品IDが一致しません"));
        }
        return Ok(existing);
    }
    // Do not reserve/overwrite this path here; Save uses atomic new-file creation.
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Ok(_) => return Err(invalid("作品保存先が既に存在します")),
        Err(e) => return Err(invalid(e)),
    }
    if sessions
        .build_runtime
        .stopping
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err(invalid("host終了中のため制作を作成できません"));
    }
    if let Some(source) = &continuation {
        source.verify_current(sessions, id)?;
    }
    let mut input = Input::new(recipe, &path)?;
    input.continuation = continuation;
    input.engine = engine;
    input.managed_profiles = managed_profiles;
    let candidate = EditSession::new(Project::empty(
        input.plan.project_id.clone(),
        input.recipe.title.clone(),
    ))
    .map_err(invalid)?;
    let session = StoredSession {
        id: uuid::Uuid::new_v4().to_string(),
        revision: 0,
        version: 0,
        snapshot: candidate.snapshot().map_err(edit_error)?.into(),
    };
    preflight(&SessionResponse::Snapshot {
        snapshot: sessions.snapshot(session.id.clone(), &candidate)?,
    })?;
    let state = State {
        phase: JobPhase::Queued,
        progress: None,
        receipts: vec![],
        child_id: None,
        expected_revision: 0,
        output: None,
        preflight: None,
        bake: None,
        saved: None,
        rendered: None,
        error: None,
    };
    state.validate(&input)?;
    let parent = StoredJob {
        id: id.into(),
        kind: JobKind::BuildVideo,
        session_id: session.id.clone(),
        version: 0,
        input: serde_json::to_vec(&input).map_err(invalid)?,
        state: serde_json::to_vec(&state).map_err(invalid)?,
    };
    sessions
        .store
        .create_build(&session, &parent)
        .map_err(storage_error)?;
    sessions
        .events
        .publish(&session.id, 0, SessionEventKind::Created, None);
    Ok(parent)
}
