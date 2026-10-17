use crate::sessions::Sessions;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use sungeki_protocol::*;
use tokio::sync::watch;
const CAPACITY: usize = 256;

pub(crate) struct Events {
    cursor: EventCursor,
    journal: VecDeque<SessionEvent>,
    changed: watch::Sender<u32>,
}
impl Events {
    pub fn new() -> Self {
        Self {
            cursor: EventCursor {
                host_instance_id: uuid::Uuid::new_v4().to_string(),
                event_seq: 0,
            },
            journal: VecDeque::new(),
            changed: watch::channel(0).0,
        }
    }
    pub fn cursor(&self) -> EventCursor {
        self.cursor.clone()
    }
    pub fn publish(
        &mut self,
        session_id: &str,
        revision: u32,
        kind: SessionEventKind,
        operation_id: Option<String>,
    ) {
        if self.cursor.event_seq == u32::MAX {
            self.cursor = EventCursor {
                host_instance_id: uuid::Uuid::new_v4().to_string(),
                event_seq: 0,
            };
            self.journal.clear();
        }
        self.cursor.event_seq += 1;
        self.journal.push_back(SessionEvent {
            event_seq: self.cursor.event_seq,
            session_id: session_id.into(),
            revision,
            kind,
            payload: EventPayload { operation_id },
        });
        if self.journal.len() > CAPACITY {
            self.journal.pop_front();
        }
        self.changed.send_replace(self.cursor.event_seq);
    }
    fn batch(&self, after: Option<&EventCursor>) -> EventBatch {
        let valid = after.filter(|c| {
            c.host_instance_id == self.cursor.host_instance_id
                && c.event_seq <= self.cursor.event_seq
                && self.cursor.event_seq - c.event_seq <= self.journal.len() as u32
        });
        EventBatch {
            cursor: self.cursor(),
            resync_required: valid.is_none(),
            events: valid
                .map(|c| {
                    self.journal
                        .iter()
                        .filter(|e| e.event_seq > c.event_seq)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

pub(crate) async fn wait(
    sessions: &Arc<Mutex<Sessions>>,
    after: Option<EventCursor>,
    wait_ms: u32,
) -> Result<EventBatch, ApiError> {
    if wait_ms > 20_000 {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "通知待機は20秒以内です",
        ));
    }
    let deadline = tokio::time::Instant::now() + Duration::from_millis(wait_ms.into());
    // Subscribe under the same lock as inspection to avoid a missed wakeup.
    let mut changed = sessions
        .lock()
        .map_err(|_| ApiError::new(ErrorCode::IoError, "ロック障害"))?
        .events
        .changed
        .subscribe();
    loop {
        let batch = sessions
            .lock()
            .map_err(|_| ApiError::new(ErrorCode::IoError, "ロック障害"))?
            .events
            .batch(after.as_ref());
        if batch.resync_required
            || !batch.events.is_empty()
            || tokio::time::Instant::now() >= deadline
        {
            return Ok(batch);
        }
        let _ = tokio::time::timeout_at(deadline, changed.changed()).await;
    }
}
