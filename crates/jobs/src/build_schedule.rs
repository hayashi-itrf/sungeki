//! Pure ordering for a durable build_video parent. The host persists the cursor
//! and child ID before dispatch, validates child outputs and performs all edits.
//! No I/O, Project mutation, automatic retry or engine ownership in this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStep {
    Prepare,
    Compose,
    Speech(u16),
    Layout,
    Validate,
    Bake,
    Save,
    Render,
    Review,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSchedule {
    speech_count: u16,
    completed: u16,
    child_id: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildScheduleError {
    InvalidCursor,
    InvalidChild,
    ChildConflict,
    Finished,
}
impl BuildSchedule {
    pub fn new(speech_count: u16) -> Result<Self, BuildScheduleError> {
        Self::restore(speech_count, 0, None)
    }
    /// Restoration does not dispatch or advance. The parent lifecycle decides
    /// whether an interrupted run can be explicitly resumed as a new job.
    pub fn restore(
        speech_count: u16,
        completed: u16,
        child_id: Option<String>,
    ) -> Result<Self, BuildScheduleError> {
        if !(1..=200).contains(&speech_count) || completed > speech_count + 8 {
            return Err(BuildScheduleError::InvalidCursor);
        }
        if let Some(id) = &child_id {
            if !valid_id(id) || completed == speech_count + 8 {
                return Err(BuildScheduleError::InvalidChild);
            }
        }
        Ok(Self {
            speech_count,
            completed,
            child_id,
        })
    }
    pub fn completed(&self) -> u16 {
        self.completed
    }
    pub fn speech_count(&self) -> u16 {
        self.speech_count
    }
    pub fn child_id(&self) -> Option<&str> {
        self.child_id.as_deref()
    }
    pub fn step(&self) -> Option<BuildStep> {
        match self.completed {
            0 => Some(BuildStep::Prepare),
            1 => Some(BuildStep::Compose),
            n if n < self.speech_count + 2 => Some(BuildStep::Speech(n - 2)),
            n => match n - self.speech_count {
                2 => Some(BuildStep::Layout),
                3 => Some(BuildStep::Validate),
                4 => Some(BuildStep::Bake),
                5 => Some(BuildStep::Save),
                6 => Some(BuildStep::Render),
                7 => Some(BuildStep::Review),
                // Eight non-speech steps in total: prepare/compose plus six below.
                _ => None,
            },
        }
    }
    /// Same ID is an idempotent reservation, never permission to launch twice.
    /// Caller commits the reservation before any external side effect.
    pub fn reserve(&mut self, id: &str) -> Result<(), BuildScheduleError> {
        if self.step().is_none() {
            return Err(BuildScheduleError::Finished);
        }
        if !valid_id(id) {
            return Err(BuildScheduleError::InvalidChild);
        }
        if self.child_id.as_deref().is_some_and(|old| old != id) {
            return Err(BuildScheduleError::ChildConflict);
        }
        self.child_id = Some(id.into());
        Ok(())
    }
    /// Only a verified successful child may be acknowledged. Failed, cancelled,
    /// stale, or uncertain children retain the cursor for inspection/recovery.
    /// Store CAS and completed-child receipts prevent a late completion replay.
    pub fn acknowledge(&mut self, id: &str) -> Result<(), BuildScheduleError> {
        if self.child_id.as_deref() != Some(id) {
            return Err(BuildScheduleError::ChildConflict);
        }
        self.completed += 1;
        self.child_id = None;
        Ok(())
    }
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
