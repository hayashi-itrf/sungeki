//! Startup reconciliation runs after the listener exists, without blocking
//! requests on potentially large collection scans.
use super::{
    Error, record,
    worker::{invalid, load},
};
use crate::{media_job_runtime::Runtime, sessions::Sessions};
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::{Duration, Instant},
};
use sungeki_storage::JobKind;
struct Active(Runtime);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.wake.notify_waiters();
    }
}
pub(crate) fn launch(shared: Arc<Mutex<Sessions>>) -> Result<(), Error> {
    let runtime = shared.lock().map_err(invalid)?.render_runtime.clone();
    let flags = runtime.flags.lock().map_err(invalid)?;
    if runtime.stopping.load(Ordering::SeqCst) {
        return Err(invalid("終了処理中です"));
    }
    runtime.active.fetch_add(1, Ordering::SeqCst);
    drop(flags);
    tokio::spawn(async move {
        let _active = Active(runtime.clone());
        let Ok(_slot) = runtime.slots.clone().acquire_owned().await else {
            return;
        };
        if let Err(e) = tokio::task::spawn_blocking(move || run(&shared, &runtime)).await {
            eprintln!("collection recovery task: {e}");
        }
    });
    Ok(())
}
fn run(shared: &Arc<Mutex<Sessions>>, runtime: &Runtime) {
    let mut after: Option<String> = None;
    loop {
        if runtime.stopping.load(Ordering::SeqCst) {
            return;
        }
        let ids = (|| {
            shared
                .lock()
                .map_err(invalid)?
                .store
                .list_jobs(Some(JobKind::Collection), None, after.as_deref(), 100)
                .map_err(invalid)
        })();
        let ids = match ids {
            Ok(ids) => ids,
            Err(e) => {
                eprintln!("collection recovery list: {e:?}");
                return;
            }
        };
        if ids.is_empty() {
            return;
        }
        for id in &ids {
            if runtime.stopping.load(Ordering::SeqCst) {
                return;
            }
            if let Err(e) = reconcile(shared, id, runtime) {
                eprintln!("collection recovery {id}: {e:?}");
            }
        }
        after = ids.last().cloned();
    }
}
fn reconcile(shared: &Arc<Mutex<Sessions>>, id: &str, runtime: &Runtime) -> Result<(), Error> {
    let observed = {
        let sessions = shared.lock().map_err(invalid)?;
        load(&sessions, id)?
    };
    let (_, before) = record::decode(&observed)?;
    if before.phase.is_terminal() {
        return Ok(());
    }
    // Do not reconcile jobs started by a client while startup scanning proceeds.
    if runtime.flags.lock().map_err(invalid)?.contains_key(id) {
        return Ok(());
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let candidate = record::recover(&observed, || {
        runtime.stopping.load(Ordering::SeqCst) || Instant::now() >= deadline
    })?;
    let mut sessions = shared.lock().map_err(invalid)?;
    let current = load(&sessions, id)?;
    let (_, state) = record::decode(&current)?;
    if state.phase.is_terminal() {
        return Ok(());
    }
    if current.input != observed.input
        || state.receipt != before.receipt
        || state.stage != before.stage
    {
        return Err(invalid("照合中に公開記録が変わりました"));
    }
    // A cancellation arriving during this read-only verification does not revert
    // a publication already proven complete. Save against the fresh CAS version.
    record::save_and_notify(&mut sessions, &current, &candidate)
}
