//! Shutdown stops admission before requesting cancellation. A timeout preserves
//! unfinished journals; it never certifies external-engine termination.
use crate::{
    build_record as build,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;
use sungeki_storage::JobKind;

pub(crate) async fn shutdown(
    shared: &Arc<Mutex<Sessions>>,
    timeout: Duration,
) -> Result<(), ApiError> {
    let (runtime, ids, mut failure) = {
        let mut sessions = shared.lock().map_err(invalid)?;
        let runtime = sessions.build_runtime.clone();
        runtime.stopping.store(true, Ordering::SeqCst);
        let mut ids = Vec::new();
        let mut after = None;
        let mut failure = None;
        loop {
            let page = sessions
                .store
                .list_jobs(Some(JobKind::BuildVideo), None, after.as_deref(), 100)
                .map_err(storage_error)?;
            if page.is_empty() {
                break;
            }
            for id in &page {
                let row = sessions
                    .store
                    .load_job(id)
                    .map_err(storage_error)?
                    .ok_or_else(|| invalid("制作記録がありません"))?;
                let (_, state) = build::decode(&row)?;
                if matches!(
                    state.phase,
                    JobPhase::Queued | JobPhase::Running | JobPhase::Cancelling
                ) {
                    ids.push(id.clone());
                    if let Err(e) = build::request_cancel(&mut sessions, &row) {
                        failure.get_or_insert(e);
                    }
                }
            }
            after = page.last().cloned();
        }
        (runtime, ids, failure)
    };
    for id in &ids {
        let result = (|| {
            let row = shared
                .lock()
                .map_err(invalid)?
                .store
                .load_job(id)
                .map_err(storage_error)?
                .ok_or_else(|| invalid("制作記録がありません"))?;
            build::cancel_owned(shared, &row)?;
            build::monitor_cancel(shared, id, timeout.min(Duration::from_secs(30)))?;
            Ok::<_, ApiError>(())
        })();
        if let Err(e) = result {
            failure.get_or_insert(e);
        }
    }
    let drained = tokio::time::timeout(timeout, async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::select! { _ = runtime.wake.notified() => {}, _ = tokio::time::sleep(Duration::from_millis(50)) => {} }
        }
    }).await.is_ok();
    if !drained {
        failure.get_or_insert_with(|| invalid("制作処理の停止確認が期限内に完了しませんでした"));
    }
    let mut sessions = shared.lock().map_err(invalid)?;
    for id in ids {
        let row = sessions
            .store
            .load_job(&id)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("制作記録がありません"))?;
        let (_, state) = build::decode(&row)?;
        if state.phase == JobPhase::Cancelled {
            continue;
        }
        match build::settle_cancel(&mut sessions, &row) {
            Ok(true) => {}
            Ok(false) => {
                failure.get_or_insert_with(|| {
                    invalid("子処理の停止確認未完了です。制作記録を保持しています")
                });
            }
            Err(e) => {
                failure.get_or_insert(e);
            }
        }
    }
    match failure {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
