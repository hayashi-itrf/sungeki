use sungeki_project::{JobPhase, Progress};

#[derive(Debug, Clone, PartialEq)]
pub struct Lifecycle {
    phase: JobPhase,
    progress: Option<Progress>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    InvalidPhase,
    InvalidProgress,
}
impl Default for Lifecycle {
    fn default() -> Self {
        Self {
            phase: JobPhase::Queued,
            progress: None,
        }
    }
}
impl Lifecycle {
    pub fn phase(&self) -> JobPhase {
        self.phase
    }
    pub fn progress(&self) -> Option<&Progress> {
        self.progress.as_ref()
    }
    pub fn start(&mut self) -> Result<(), TransitionError> {
        if self.phase != JobPhase::Queued {
            return Err(TransitionError::InvalidPhase);
        }
        self.phase = JobPhase::Running;
        Ok(())
    }
    pub fn update_progress(&mut self, progress: Progress) -> Result<(), TransitionError> {
        if self.phase != JobPhase::Running {
            return Err(TransitionError::InvalidPhase);
        }
        validate_progress(&progress)?;
        self.progress = Some(progress);
        Ok(())
    }
    /// Idempotent cancellation: terminal work is never rewritten. Cleanup must
    /// finish before acknowledge_cancel; requesting cancellation is not completion.
    pub fn request_cancel(&mut self) -> bool {
        if self.phase.is_terminal() {
            return false;
        }
        self.phase = JobPhase::Cancelling;
        true
    }
    pub fn acknowledge_cancel(&mut self) -> Result<(), TransitionError> {
        if self.phase != JobPhase::Cancelling {
            return Err(TransitionError::InvalidPhase);
        }
        self.phase = JobPhase::Cancelled;
        Ok(())
    }
    /// The owning scheduler serializes completion against cancellation. If cancel
    /// won first, callers must discard success artifacts and finish cleanup.
    pub fn finish(&mut self, succeeded: bool) -> Result<(), TransitionError> {
        if self.phase != JobPhase::Running {
            return Err(TransitionError::InvalidPhase);
        }
        self.phase = if succeeded {
            JobPhase::Succeeded
        } else {
            JobPhase::Failed
        };
        Ok(())
    }
    /// Recovery never silently starts queued/running/cancelling work again.
    /// Terminal persisted results retain their phase; interrupted work is retried
    /// only as a new job. Input/results remain the scheduler/store's responsibility.
    pub fn recover(phase: JobPhase, progress: Option<Progress>) -> Result<Self, TransitionError> {
        Self::restore(
            if phase.is_terminal() {
                phase
            } else {
                JobPhase::Interrupted
            },
            progress,
        )
    }
    pub fn restore(phase: JobPhase, progress: Option<Progress>) -> Result<Self, TransitionError> {
        if let Some(value) = &progress {
            validate_progress(value)?;
        }
        Ok(Self { phase, progress })
    }
}

fn validate_progress(progress: &Progress) -> Result<(), TransitionError> {
    if progress.stage.trim().is_empty()
        || progress
            .fraction
            .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
    {
        return Err(TransitionError::InvalidProgress);
    }
    Ok(())
}
