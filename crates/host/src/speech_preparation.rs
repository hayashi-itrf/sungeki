//! Bound slow filesystem preparation before durable job creation. A replay must
//! not be reported as a definitive rejection while its original call can commit.
use crate::speech_state::invalid;
use std::sync::{Arc, Mutex};
use sungeki_protocol::{ApiError, ErrorCode};
#[derive(Clone, PartialEq, Eq)]
struct Key {
    job: String,
    session: String,
    input_hash: String,
}
#[derive(Clone, Default)]
pub(crate) struct Preparations(Arc<Mutex<Option<Key>>>);
pub(crate) struct Preparation(Preparations);
impl Preparations {
    pub fn is_active(&self) -> Result<bool, ApiError> {
        Ok(self.0.lock().map_err(invalid)?.is_some())
    }
    /// Transfer the existing guard without releasing the original request key.
    /// Replays must stay uncertain until the durable record becomes visible.
    pub fn adopt(
        &self,
        preparation: Preparation,
        job: &str,
        session: &str,
    ) -> Result<Preparation, ApiError> {
        let same_registry = Arc::ptr_eq(&self.0, &preparation.0.0);
        let matches = self
            .0
            .lock()
            .map_err(invalid)?
            .as_ref()
            .is_some_and(|key| key.job == job && key.session == session);
        if !same_registry || !matches {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "合成準備の所有者が一致しません",
            ));
        }
        Ok(preparation)
    }
    pub fn contains(&self, job: &str) -> Result<bool, ApiError> {
        Ok(self
            .0
            .lock()
            .map_err(invalid)?
            .as_ref()
            .is_some_and(|k| k.job == job))
    }
    pub fn claim(
        &self,
        job: &str,
        session: &str,
        input_hash: &str,
    ) -> Result<Preparation, ApiError> {
        if uuid::Uuid::parse_str(job)
            .ok()
            .is_none_or(|value| value.to_string() != job)
        {
            return Err(ApiError::new(
                ErrorCode::ValidationFailed,
                "job_idは標準形式のUUIDが必要です",
            ));
        }
        let key = Key {
            job: job.into(),
            session: session.into(),
            input_hash: input_hash.into(),
        };
        let mut current = self.0.lock().map_err(invalid)?;
        if let Some(active) = current.as_ref() {
            let (code, message) = if active == &key {
                (
                    ErrorCode::HostUnavailable,
                    "同じ合成要求の保存先を確認中です。元の要求を保持して再確認してください",
                )
            } else if active.job == job {
                (
                    ErrorCode::Conflict,
                    "確認中のjob_idが別の要求で使われています",
                )
            } else {
                (
                    ErrorCode::CapabilityUnavailable,
                    "別の合成要求の保存先を確認中です。このジョブはまだ作成していません",
                )
            };
            let mut error = ApiError::new(code, message);
            error.retryable = active == &key;
            error.details.insert(
                "job_created".into(),
                if active == &key { "unknown" } else { "false" }.into(),
            );
            return Err(error);
        }
        *current = Some(key);
        Ok(Preparation(self.clone()))
    }
}
impl Drop for Preparation {
    fn drop(&mut self) {
        *self.0.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handoff_keeps_original_replay_key_and_excludes_other_requests() {
        let preparations = Preparations::default();
        let id = uuid::Uuid::new_v4().to_string();
        let other = uuid::Uuid::new_v4().to_string();
        let held = preparations
            .claim(&id, "session", "original request")
            .unwrap();
        let held = preparations.adopt(held, &id, "session").unwrap();
        assert!(preparations.contains(&id).unwrap());
        let replay = preparations
            .claim(&id, "session", "original request")
            .err()
            .unwrap();
        assert_eq!(replay.code, ErrorCode::HostUnavailable);
        assert!(replay.retryable);
        assert_eq!(replay.details["job_created"], "unknown");
        assert_eq!(
            preparations
                .claim(&id, "session", "changed request")
                .err()
                .unwrap()
                .code,
            ErrorCode::Conflict
        );
        assert_eq!(
            preparations
                .claim(&other, "session", "other")
                .err()
                .unwrap()
                .code,
            ErrorCode::CapabilityUnavailable
        );
        drop(held);
        assert!(!preparations.contains(&id).unwrap());
        assert!(preparations.claim(&other, "session", "other").is_ok());
    }
    #[test]
    fn handoff_rejects_wrong_registry_or_target_without_clearing_other_owner() {
        let first = Preparations::default();
        let second = Preparations::default();
        let id = uuid::Uuid::new_v4().to_string();
        let held = first.claim(&id, "session", "request").unwrap();
        let second_held = second.claim(&id, "session", "request").unwrap();
        assert!(second.adopt(held, &id, "session").is_err());
        assert!(second.contains(&id).unwrap());
        assert!(!first.contains(&id).unwrap());
        assert!(second.adopt(second_held, &id, "wrong session").is_err());
        assert!(!second.contains(&id).unwrap());
    }
    #[test]
    fn repeated_pending_start_stays_uncertain_and_other_work_is_bounded() {
        let preparations = Preparations::default();
        let id = uuid::Uuid::new_v4().to_string();
        let other = uuid::Uuid::new_v4().to_string();
        let held = preparations.claim(&id, "session", "hash").unwrap();
        let replay = preparations.claim(&id, "session", "hash").err().unwrap();
        assert_eq!(replay.code, ErrorCode::HostUnavailable);
        assert_eq!(replay.details["job_created"], "unknown");
        assert!(replay.retryable);
        assert_eq!(
            preparations
                .claim(&id, "session", "different")
                .err()
                .unwrap()
                .code,
            ErrorCode::Conflict
        );
        let rejected = preparations.claim(&other, "session", "hash").err().unwrap();
        assert_eq!(rejected.code, ErrorCode::CapabilityUnavailable);
        assert_eq!(rejected.details["job_created"], "false");
        drop(held);
        assert!(preparations.claim(&other, "session", "hash").is_ok());
        assert_eq!(
            preparations
                .claim("../invalid", "session", "hash")
                .err()
                .unwrap()
                .code,
            ErrorCode::ValidationFailed
        );
    }
}
