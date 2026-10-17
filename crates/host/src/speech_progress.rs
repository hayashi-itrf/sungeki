//! Cheap worker-side reporting; only the job owner persists observed stages.
use crate::{sessions::Sessions, speech_state::invalid};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, Ordering},
};
use sungeki_project::{JobPhase, Progress};
use sungeki_protocol::ApiError;

#[derive(Clone, Copy)]
#[repr(u8)]
pub(crate) enum Stage {
    Inspect,
    Start,
    Recheck,
    Synthesize,
    Verify,
    Stop,
    Save,
}
#[derive(Clone, Default)]
pub(crate) struct Tracker(Arc<AtomicU8>);
impl Tracker {
    pub(crate) fn set(&self, stage: Stage) {
        self.0.store(stage as u8, Ordering::Release);
    }
    fn snapshot(&self) -> Progress {
        let stage = match self.0.load(Ordering::Acquire) {
            0 => "speech.inspect_originals",
            1 => "speech.start_engine",
            2 => "speech.recheck_originals",
            3 => "speech.synthesize",
            4 => "speech.verify_output",
            5 => "speech.stop_engine",
            _ => "speech.save_audio",
        };
        Progress {
            stage: stage.into(),
            fraction: None,
        }
    }
    pub(crate) fn persist(&self, shared: &Arc<Mutex<Sessions>>, id: &str) -> Result<(), ApiError> {
        let mut sessions = shared.lock().map_err(invalid)?;
        let record = crate::speech_failure::load(&sessions, id)?;
        let (_, mut state) = crate::speech_journal::decode_input(&record)?;
        // Never overwrite cancellation/terminal state, and avoid writes every poll.
        let progress = Some(self.snapshot());
        if state.phase == JobPhase::Running && state.progress != progress {
            state.progress = progress;
            crate::speech_journal::save(&mut sessions, &record, &state)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech_journal_tests::{Fixture, id, seed};

    #[test]
    fn progress_is_durable_deduplicated_and_does_not_revive_cancellation() {
        let fixture = Fixture::new();
        let mut sessions = fixture.open();
        let (sid, input) = seed(&mut sessions);
        let job = crate::speech_journal::create(&mut sessions, id(), sid, &input).unwrap();
        let (_, mut state) = crate::speech_journal::decode_input(&job).unwrap();
        state.phase = JobPhase::Running;
        crate::speech_journal::save(&mut sessions, &job, &state).unwrap();
        let shared = Arc::new(Mutex::new(sessions));
        let tracker = Tracker::default();
        tracker.persist(&shared, &job.id).unwrap();
        let first = crate::speech_failure::load(&shared.lock().unwrap(), &job.id).unwrap();
        tracker.persist(&shared, &job.id).unwrap();
        let repeat = crate::speech_failure::load(&shared.lock().unwrap(), &job.id).unwrap();
        assert_eq!(first.version, repeat.version);
        tracker.set(Stage::Synthesize);
        tracker.persist(&shared, &job.id).unwrap();
        let last = crate::speech_failure::load(&shared.lock().unwrap(), &job.id).unwrap();
        let (_, mut state) = crate::speech_journal::decode_input(&last).unwrap();
        assert_eq!(state.progress.as_ref().unwrap().stage, "speech.synthesize");
        assert_eq!(state.progress.as_ref().unwrap().fraction, None);
        state.phase = JobPhase::Cancelling;
        crate::speech_journal::save(&mut shared.lock().unwrap(), &last, &state).unwrap();
        let cancelled = crate::speech_failure::load(&shared.lock().unwrap(), &job.id).unwrap();
        tracker.set(Stage::Save);
        tracker.persist(&shared, &job.id).unwrap();
        let retained = crate::speech_failure::load(&shared.lock().unwrap(), &job.id).unwrap();
        assert_eq!(cancelled.version, retained.version);
        assert_eq!(cancelled.state, retained.state);
        drop(shared);
        let reopened = fixture.open();
        let restored = crate::speech_failure::load(&reopened, &job.id).unwrap();
        let (_, state) = crate::speech_journal::decode_input(&restored).unwrap();
        assert_eq!(state.phase, JobPhase::Interrupted);
        assert_eq!(state.progress.unwrap().stage, "speech.synthesize");
    }
    #[test]
    fn late_stage_updates_do_not_change_terminal_records() {
        for phase in [JobPhase::Failed, JobPhase::Cancelled, JobPhase::Interrupted] {
            let fixture = Fixture::new();
            let mut sessions = fixture.open();
            let (sid, input) = seed(&mut sessions);
            let job = crate::speech_journal::create(&mut sessions, id(), sid, &input).unwrap();
            let (_, mut state) = crate::speech_journal::decode_input(&job).unwrap();
            state.phase = if phase == JobPhase::Cancelled {
                JobPhase::Cancelling
            } else {
                JobPhase::Running
            };
            crate::speech_journal::save(&mut sessions, &job, &state).unwrap();
            let job = crate::speech_failure::load(&sessions, &job.id).unwrap();
            state.phase = phase;
            if phase == JobPhase::Failed {
                state.error = Some(invalid("fixture failure"));
            }
            crate::speech_journal::save(&mut sessions, &job, &state).unwrap();
            let shared = Arc::new(Mutex::new(sessions));
            let before = crate::speech_failure::load(&shared.lock().unwrap(), &job.id).unwrap();
            let tracker = Tracker::default();
            tracker.set(Stage::Save);
            tracker.persist(&shared, &job.id).unwrap();
            let after = crate::speech_failure::load(&shared.lock().unwrap(), &job.id).unwrap();
            assert_eq!(before.version, after.version);
            assert_eq!(before.state, after.state);
        }
    }
}
