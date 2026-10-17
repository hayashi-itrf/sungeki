mod cached_project;
use crate::{Command, apply_batch};
use cached_project::CachedProject;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use sungeki_project::{Project, ValidationError, validate};

const MAX_OPERATIONS: usize = 100;
const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
pub enum HistoryError {
    Conflict,
    Empty,
    RevisionLimit,
    Validation(ValidationError),
}
impl From<ValidationError> for HistoryError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    project: CachedProject,
    bytes: usize,
    operation_id: String,
}

// Pure, in-memory candidate state. The future host must persist a cloned
// candidate and its receipt before replacing the committed instance.
#[derive(Clone, Serialize, Deserialize)]
pub struct History {
    project: Project,
    revision: u32,
    undo: VecDeque<Entry>,
    redo: VecDeque<Entry>,
}
impl History {
    pub fn new(project: Project) -> Result<Self, ValidationError> {
        validate(&project)?;
        Ok(Self {
            project,
            revision: 0,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
        })
    }
    pub fn undo_operation_id(&self) -> Option<&str> {
        self.undo.back().map(|e| e.operation_id.as_str())
    }
    pub fn redo_operation_id(&self) -> Option<&str> {
        self.redo.back().map(|e| e.operation_id.as_str())
    }
    pub(crate) fn tag_latest(&mut self, operation_id: &str) {
        if let Some(entry) = self.undo.back_mut() {
            entry.operation_id = operation_id.into();
        }
    }
    pub(crate) fn validate_restored(&mut self) -> Result<(), ValidationError> {
        validate(&self.project)?;
        for entry in self.undo.iter_mut().chain(&mut self.redo) {
            validate(entry.project.project())?;
            entry.bytes = entry.project.bytes();
        }
        self.trim();
        Ok(())
    }
    /// Current state and every retained Undo/Redo state, for save inventories.
    pub fn retained_projects(&self) -> impl Iterator<Item = &Project> {
        std::iter::once(&self.project).chain(
            self.undo
                .iter()
                .chain(&self.redo)
                .map(|e| e.project.project()),
        )
    }
    /// Physical relocation is not an undoable edit: Undo must not restore the
    /// old source path. Bump revision so concurrent clients refresh their state.
    pub(crate) fn relocate_audio(
        &mut self,
        expected: u32,
        relocations: &crate::AudioRelocations,
    ) -> Result<bool, HistoryError> {
        self.expect(expected)?;
        let mut candidate = self.clone();
        candidate.project = relocations.relocate_project(&self.project)?;
        let mut changed = candidate.project != self.project;
        for entry in candidate.undo.iter_mut().chain(&mut candidate.redo) {
            let project = relocations.relocate_project(entry.project.project())?;
            changed |= &project != entry.project.project();
            entry.project = CachedProject::new(project)?;
        }
        if !changed {
            return Ok(false);
        }
        candidate.revision = self.next_revision()?;
        candidate.validate_restored()?;
        *self = candidate;
        Ok(true)
    }
    pub fn project(&self) -> &Project {
        &self.project
    }
    pub fn revision(&self) -> u32 {
        self.revision
    }
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
    fn expect(&self, expected: u32) -> Result<(), HistoryError> {
        if expected == self.revision {
            Ok(())
        } else {
            Err(HistoryError::Conflict)
        }
    }
    fn snapshot(&self) -> Result<Entry, HistoryError> {
        let project = CachedProject::new(self.project.clone())?;
        let bytes = project.bytes();
        Ok(Entry {
            project,
            bytes,
            operation_id: String::new(),
        })
    }
    fn next_revision(&self) -> Result<u32, HistoryError> {
        self.revision
            .checked_add(1)
            .ok_or(HistoryError::RevisionLimit)
    }
    pub fn apply(&mut self, expected: u32, commands: &[Command]) -> Result<bool, HistoryError> {
        self.expect(expected)?;
        let candidate = apply_batch(&self.project, commands)?;
        if candidate == self.project {
            return Ok(false);
        }
        let revision = self.next_revision()?;
        let snapshot = self.snapshot()?;
        self.undo.push_back(snapshot);
        self.redo.clear();
        self.project = candidate;
        self.revision = revision;
        self.trim();
        Ok(true)
    }
    pub fn undo(&mut self, expected: u32) -> Result<(), HistoryError> {
        self.expect(expected)?;
        if self.undo.is_empty() {
            return Err(HistoryError::Empty);
        }
        let revision = self.next_revision()?;
        let mut snapshot = self.snapshot()?;
        let entry = self.undo.pop_back().unwrap();
        snapshot.operation_id = entry.operation_id;
        self.redo.push_back(snapshot);
        self.project = entry.project.into_project();
        self.revision = revision;
        self.trim();
        Ok(())
    }
    pub fn redo(&mut self, expected: u32) -> Result<(), HistoryError> {
        self.expect(expected)?;
        if self.redo.is_empty() {
            return Err(HistoryError::Empty);
        }
        let revision = self.next_revision()?;
        let mut snapshot = self.snapshot()?;
        let entry = self.redo.pop_back().unwrap();
        snapshot.operation_id = entry.operation_id;
        self.undo.push_back(snapshot);
        self.project = entry.project.into_project();
        self.revision = revision;
        self.trim();
        Ok(())
    }
    fn trim(&mut self) {
        while self.undo.len() + self.redo.len() > MAX_OPERATIONS
            || self
                .undo
                .iter()
                .chain(&self.redo)
                .map(|e| e.bytes)
                .sum::<usize>()
                > MAX_BYTES
        {
            if !self.undo.is_empty() {
                self.undo.pop_front();
            } else {
                self.redo.pop_front();
            }
        }
    }
}
