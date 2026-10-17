//! Foreground media work preempts a cooperative, low-priority proxy worker.
//! A slot is never stolen: the proxy must stop and clean up before releasing it.
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{AcquireError, OwnedSemaphorePermit, Semaphore, TryAcquireError};

#[derive(Default)]
struct State {
    waiting: usize,
    background: Option<Weak<AtomicBool>>,
}
pub(crate) struct Slots {
    semaphore: Arc<Semaphore>,
    state: Mutex<State>,
}
impl Default for Slots {
    fn default() -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(1)),
            state: Default::default(),
        }
    }
}
struct Waiting(Arc<Slots>);
impl Drop for Waiting {
    fn drop(&mut self) {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .waiting -= 1;
    }
}
pub(crate) struct BackgroundSlot {
    /// Separate from user cancellation: preemption permits the job to retry later.
    pub interrupted: Arc<AtomicBool>,
    slots: Arc<Slots>,
    _permit: OwnedSemaphorePermit,
}
impl Drop for BackgroundSlot {
    fn drop(&mut self) {
        let mut state = self.slots.state.lock().unwrap_or_else(|e| e.into_inner());
        if state
            .background
            .as_ref()
            .is_some_and(|flag| flag.ptr_eq(&Arc::downgrade(&self.interrupted)))
        {
            state.background = None;
        }
    }
}
impl Slots {
    fn foreground(self: &Arc<Self>) -> Waiting {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.waiting += 1;
        if let Some(flag) = state.background.as_ref().and_then(Weak::upgrade) {
            flag.store(true, Ordering::Release);
        }
        Waiting(self.clone())
    }
    pub async fn acquire_owned(self: Arc<Self>) -> Result<OwnedSemaphorePermit, AcquireError> {
        let _waiting = self.foreground();
        self.semaphore.clone().acquire_owned().await
    }
    pub fn try_acquire_owned(self: Arc<Self>) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        let _waiting = self.foreground();
        self.semaphore.clone().try_acquire_owned()
    }
    pub fn interrupt_background(&self) {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(flag) = state.background.as_ref().and_then(Weak::upgrade) {
            flag.store(true, Ordering::Release);
        }
    }
    /// Try only while no foreground waiter exists. Background work never enters
    /// the semaphore FIFO ahead of a later render request.
    pub async fn acquire_background(
        self: Arc<Self>,
        cancelled: &AtomicBool,
        stopping: &AtomicBool,
    ) -> Option<BackgroundSlot> {
        loop {
            if cancelled.load(Ordering::Acquire) || stopping.load(Ordering::Acquire) {
                return None;
            }
            {
                let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                if cancelled.load(Ordering::Acquire) || stopping.load(Ordering::Acquire) {
                    return None;
                }
                if state.waiting == 0 {
                    if let Ok(permit) = self.semaphore.clone().try_acquire_owned() {
                        let interrupted = Arc::new(AtomicBool::new(false));
                        state.background = Some(Arc::downgrade(&interrupted));
                        return Some(BackgroundSlot {
                            interrupted,
                            slots: self.clone(),
                            _permit: permit,
                        });
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }
    #[cfg(test)]
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}

#[cfg(test)]
#[path = "media_job_priority_tests.rs"]
mod tests;
