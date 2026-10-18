use crate::sessions::Sessions;
use std::sync::{Arc, Mutex, atomic::Ordering};
use std::time::Duration;

/// Do not tear down the runtime while an owned engine/worker is settling.
pub(crate) async fn cancel_and_wait(shared: &Arc<Mutex<Sessions>>) {
    crate::http_engine::begin_shutdown();
    if let Err(e) = crate::speech_runtime::cancel_all(shared) {
        eprintln!("speech shutdown: {e}");
    }
    let (speech, review) = {
        let s = shared.lock().unwrap();
        (s.speech_runtime.clone(), s.review_runtime.clone())
    };
    for flag in review.cancellations.lock().unwrap().values() {
        flag.store(true, Ordering::SeqCst);
    }
    while speech.active.load(Ordering::SeqCst) != 0 || review.active.load(Ordering::SeqCst) != 0 {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    crate::http_engine::shutdown().await;
}
