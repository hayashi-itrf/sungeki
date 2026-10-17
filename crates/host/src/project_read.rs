use crate::sessions::Sessions;
use std::collections::{HashMap, HashSet};
use sungeki_project::{Clip, Project};
use sungeki_protocol::*;
fn invalid(message: &str) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message)
}
fn ids(
    values: Option<Vec<String>>,
    existing: impl Iterator<Item = String>,
) -> Result<Option<HashSet<String>>, ApiError> {
    let Some(values) = values else {
        return Ok(None);
    };
    let known: HashSet<_> = existing.collect();
    let values: HashSet<_> = values.into_iter().collect();
    if let Some(missing) = values.iter().find(|id| !known.contains(*id)) {
        let mut error = ApiError::new(ErrorCode::NotFound, "指定した項目が見つかりません");
        error.details.insert("id".into(), missing.clone());
        return Err(error);
    }
    Ok(Some(values))
}
fn page(total: usize, offset: u32, limit: u32) -> Result<(usize, usize, Option<u32>), ApiError> {
    if offset as usize > total {
        return Err(invalid("ページが範囲外です"));
    }
    let end = (offset as usize + limit as usize).min(total);
    Ok((offset as usize, end, (end < total).then_some(end as u32)))
}
impl Sessions {
    pub(crate) fn read_project(
        &self,
        id: String,
        revision: Option<u32>,
        query: ProjectQuery,
    ) -> Result<SessionResponse, ApiError> {
        if !(1..=50).contains(&query.limit) {
            return Err(invalid("件数は1〜50です"));
        }
        if query.offset > 0 && revision.is_none() {
            return Err(invalid("続きの取得にはexpected_revisionが必要です"));
        }
        if matches!(query.view, ProjectView::Full | ProjectView::Summary)
            && (query.scene_ids.is_some() || query.clip_ids.is_some() || query.offset != 0)
        {
            return Err(invalid(
                "絞り込みとページ指定にはscenesまたはtimelineを選択してください",
            ));
        }
        let (_, session) = self.load(&id)?;
        let current_revision = session.history().revision();
        if revision.is_some_and(|r| r != current_revision) {
            let mut error = ApiError::new(ErrorCode::Conflict, "取得対象が更新されています");
            error
                .details
                .insert("current_revision".into(), current_revision.to_string());
            return Err(error);
        }
        if matches!(query.view, ProjectView::Full) {
            return Ok(SessionResponse::Snapshot {
                snapshot: self.snapshot(id, &session)?,
            });
        }
        let project = session.history().project();
        let scenes = ids(query.scene_ids, project.scenes.iter().map(|s| s.id.clone()))?;
        let clips = ids(query.clip_ids, project.clips.iter().map(|c| c.id.clone()))?;
        let group_by_clip: HashMap<_, _> = project
            .groups
            .iter()
            .flat_map(|g| {
                g.clip_ids
                    .iter()
                    .map(move |id| (id.as_str(), g.id.as_str()))
            })
            .collect();
        let data = match query.view {
            ProjectView::Summary => {
                let snapshot = self.snapshot(id.clone(), &session)?;
                ProjectViewData::Summary {
                    title: project.title.clone(),
                    canvas: project.canvas.clone(),
                    duration_us: project.duration().unwrap(),
                    counts: ProjectCounts {
                        tracks: project.tracks.len(),
                        scenes: project.scenes.len(),
                        groups: project.groups.len(),
                        clips: project.clips.len(),
                        assets: project.assets.len(),
                        characters: project.characters.len(),
                    },
                    dirty: snapshot.dirty,
                    document_path: snapshot.document_path,
                }
            }
            ProjectView::Scenes => {
                let mut selected: Vec<_> = project
                    .scenes
                    .iter()
                    .filter(|s| scenes.as_ref().is_none_or(|ids| ids.contains(&s.id)))
                    .filter(|s| {
                        clips.as_ref().is_none_or(|ids| {
                            project
                                .clips
                                .iter()
                                .any(|c| ids.contains(&c.id) && c.scene_id.as_ref() == Some(&s.id))
                        })
                    })
                    .collect();
                selected.sort_by_key(|s| (s.start_us, &s.id));
                let total = selected.len();
                let (start, end, next_offset) = page(total, query.offset, query.limit)?;
                let entries = selected[start..end]
                    .iter()
                    .map(|s| {
                        let members: Vec<_> = project
                            .clips
                            .iter()
                            .filter(|c| c.scene_id.as_ref() == Some(&s.id))
                            .collect();
                        let group_count = members
                            .iter()
                            .filter_map(|c| group_by_clip.get(c.id.as_str()))
                            .collect::<HashSet<_>>()
                            .len();
                        SceneEntry {
                            scene: (*s).clone(),
                            clip_count: members.len(),
                            group_count,
                        }
                    })
                    .collect();
                ProjectViewData::Scenes {
                    entries,
                    total,
                    next_offset,
                }
            }
            ProjectView::Timeline => timeline(
                project,
                scenes.as_ref(),
                clips.as_ref(),
                &group_by_clip,
                query.offset,
                query.limit,
            )?,
            ProjectView::Full => unreachable!(),
        };
        Ok(SessionResponse::ProjectRead {
            result: ProjectReadResult {
                session_id: id,
                project_id: project.id.clone(),
                revision: current_revision,
                event_cursor: self.events.cursor(),
                data,
            },
        })
    }
}
fn timeline(
    p: &Project,
    scenes: Option<&HashSet<String>>,
    clips: Option<&HashSet<String>>,
    groups: &HashMap<&str, &str>,
    offset: u32,
    limit: u32,
) -> Result<ProjectViewData, ApiError> {
    let selected: Vec<(usize, &Clip)> = p
        .clips
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            clips.is_none_or(|ids| ids.contains(&c.id))
                && scenes.is_none_or(|ids| c.scene_id.as_ref().is_some_and(|id| ids.contains(id)))
        })
        .collect();
    let total = selected.len();
    let (start, end, next_offset) = page(total, offset, limit)?;
    let track_order: HashMap<_, _> = p
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();
    let entries: Vec<_> = selected[start..end]
        .iter()
        .map(|(index, c)| TimelineEntry {
            clip: (*c).clone(),
            clip_order: *index,
            track_order: track_order[c.track_id.as_str()],
            group_id: groups.get(c.id.as_str()).map(|id| (*id).into()),
        })
        .collect();
    let used: HashSet<_> = entries.iter().map(|e| e.clip.track_id.as_str()).collect();
    let tracks = p
        .tracks
        .iter()
        .filter(|t| used.contains(t.id.as_str()))
        .cloned()
        .collect();
    Ok(ProjectViewData::Timeline {
        entries,
        tracks,
        total,
        next_offset,
    })
}
