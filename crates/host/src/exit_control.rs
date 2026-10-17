//! Connection ownership and admission share one lock with the shutdown decision.
use crate::sessions::Sessions;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use sungeki_protocol::{ApiError, ErrorCode, ExitAction, ExitStatus};

#[derive(Default)]
struct State {
    peers: HashMap<u64, Option<i32>>,
    pending: usize,
    next: u64,
    stopping: bool,
}
#[derive(Default)]
pub(crate) struct Control {
    state: Mutex<State>,
    pub fast_idle: AtomicBool,
    pub stop: AtomicBool,
    pub wake: tokio::sync::Notify,
}
pub(crate) struct Connection {
    control: Arc<Control>,
    id: u64,
    pub pid: Option<i32>,
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.control.state.lock().unwrap().peers.remove(&self.id);
        self.control.wake.notify_one();
    }
}
pub(crate) struct RequestGuard(Arc<Control>);
impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().pending -= 1;
        self.0.wake.notify_one();
    }
}
impl Control {
    pub fn connect(self: &Arc<Self>, pid: Option<i32>) -> Option<Connection> {
        let mut s = self.state.lock().unwrap();
        if s.stopping {
            return None;
        }
        s.next += 1;
        let id = s.next;
        s.peers.insert(id, pid);
        Some(Connection {
            control: self.clone(),
            id,
            pid,
        })
    }
    pub fn admit(self: &Arc<Self>) -> Result<RequestGuard, ApiError> {
        let mut s = self.state.lock().unwrap();
        if s.stopping {
            return Err(ApiError::new(ErrorCode::HostUnavailable, "終了処理中です"));
        }
        s.pending += 1;
        Ok(RequestGuard(self.clone()))
    }
    pub fn inspect(
        &self,
        connection: &Connection,
        sessions: &Sessions,
        action: ExitAction,
    ) -> ExitStatus {
        let mut s = self.state.lock().unwrap();
        let count = |n: &std::sync::atomic::AtomicUsize| n.load(Ordering::SeqCst) as u32;
        let active_jobs = count(&sessions.build_runtime.active)
            + count(&sessions.render_runtime.active)
            + count(&sessions.loudness_runtime.active)
            + count(&sessions.speech_runtime.active)
            + count(&sessions.review_runtime.active);
        let others = s
            .peers
            .iter()
            .filter(|(id, pid)| {
                **id != connection.id && (connection.pid.is_none() || **pid != connection.pid)
            })
            .count() as u32;
        if matches!(action, ExitAction::WhenIdle) {
            self.fast_idle.store(true, Ordering::SeqCst);
        }
        if (matches!(action, ExitAction::CancelAndStop)
            || matches!(action, ExitAction::StopIfIdle) && active_jobs == 0)
            && others == 0
            && s.pending == 0
        {
            s.stopping = true;
            self.stop.store(true, Ordering::SeqCst);
        }
        let result = ExitStatus {
            active_jobs,
            other_clients: others,
            pending_requests: s.pending as u32,
            stopping: s.stopping,
        };
        self.wake.notify_one();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (std::path::PathBuf, Sessions) {
        let root = std::env::temp_dir().join(format!("next-exit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let sessions = Sessions::open(&root.join("recovery.sqlite")).unwrap();
        (root, sessions)
    }
    #[test]
    fn stop_is_atomic_with_admission_and_other_clients() {
        let (root, sessions) = fixture();
        let control = Arc::new(Control::default());
        let owner = control.connect(Some(1)).unwrap();
        let own_lease = control.connect(Some(1)).unwrap();
        let other = control.connect(Some(2)).unwrap();
        let status = control.inspect(&owner, &sessions, ExitAction::CancelAndStop);
        assert_eq!(status.other_clients, 1);
        assert!(!status.stopping);
        drop(other);
        let request = control.admit().unwrap();
        assert!(
            !control
                .inspect(&owner, &sessions, ExitAction::CancelAndStop)
                .stopping
        );
        drop(request);
        assert!(
            control
                .inspect(&owner, &sessions, ExitAction::CancelAndStop)
                .stopping
        );
        assert!(control.connect(Some(3)).is_none());
        assert!(control.admit().is_err());
        drop(own_lease);
        drop(owner);
        drop(sessions);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn background_keeps_jobs_and_unknown_peers_are_protected() {
        let (root, sessions) = fixture();
        let control = Arc::new(Control::default());
        let owner = control.connect(None).unwrap();
        let other = control.connect(None).unwrap();
        sessions.render_runtime.active.store(1, Ordering::SeqCst);
        let status = control.inspect(&owner, &sessions, ExitAction::WhenIdle);
        assert_eq!(status.active_jobs, 1);
        drop(other);
        assert!(
            !control
                .inspect(&owner, &sessions, ExitAction::StopIfIdle)
                .stopping
        );
        let other = control.connect(None).unwrap();
        assert_eq!(status.other_clients, 1);
        assert!(!status.stopping);
        assert!(control.fast_idle.load(Ordering::SeqCst));
        assert!(!sessions.render_runtime.stopping.load(Ordering::SeqCst));
        assert!(
            !control
                .inspect(&owner, &sessions, ExitAction::CancelAndStop)
                .stopping
        );
        drop(other);
        drop(owner);
        drop(sessions);
        std::fs::remove_dir_all(root).unwrap();
    }
}
