use super::super::super::speech_input::preparation;
use super::setup;
use crate::build_record;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

fn await_cancel(flag: &AtomicBool, finished: &AtomicBool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !flag.load(Ordering::Acquire) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        flag.load(Ordering::Acquire),
        "inspection never received cancellation"
    );
    finished.store(true, Ordering::Release);
}

#[tokio::test]
async fn persisted_parent_cancel_and_host_shutdown_reach_inspection_before_return() {
    for shutdown in [false, true] {
        let (root, sessions, parent, _, child_id) = setup();
        let shared = Arc::new(Mutex::new(sessions));
        let finished = Arc::new(AtomicBool::new(false));
        let done = finished.clone();
        let context = shared.clone();
        let expected = parent.clone();
        let (send, receive) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            preparation::run(&context, &expected, move |flag| {
                send.send(()).unwrap();
                await_cancel(&flag, &done);
                Ok(())
            })
            .await
        });
        receive.await.unwrap();
        if shutdown {
            shared
                .lock()
                .unwrap()
                .build_runtime
                .stopping
                .store(true, Ordering::Release);
        } else {
            build_record::request_cancel(&mut shared.lock().unwrap(), &parent).unwrap();
        }
        let result = tokio::time::timeout(Duration::from_secs(4), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.unwrap_err().code,
            sungeki_protocol::ErrorCode::Cancelled
        );
        assert!(
            finished.load(Ordering::Acquire),
            "must await inspection cleanup"
        );
        let mut sessions = shared.lock().unwrap();
        if !shutdown {
            let row = sessions.store.load_job(&parent.id).unwrap().unwrap();
            assert!(build_record::decode(&row).unwrap().1.child_id.is_none());
            assert!(build_record::settle_cancel(&mut sessions, &row).unwrap());
        }
        assert!(sessions.store.load_job(&child_id).unwrap().is_none());
        assert_eq!(sessions.speech_runtime.active.load(Ordering::SeqCst), 0);
        drop(sessions);
        drop(shared);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[tokio::test]
async fn dropping_observer_signals_inspection_and_already_cancelled_parent_never_starts_work() {
    let (root, sessions, parent, _, _) = setup();
    let shared = Arc::new(Mutex::new(sessions));
    let context = shared.clone();
    let expected = parent.clone();
    let (send, receive) = tokio::sync::oneshot::channel();
    let (stopped, cleanup) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        preparation::run(&context, &expected, move |flag| {
            send.send(()).unwrap();
            await_cancel(&flag, &AtomicBool::new(false));
            stopped.send(()).unwrap();
            Ok(())
        })
        .await
    });
    receive.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(4), cleanup)
        .await
        .unwrap()
        .unwrap();
    build_record::request_cancel(&mut shared.lock().unwrap(), &parent).unwrap();
    let called = Arc::new(AtomicBool::new(false));
    let marker = called.clone();
    assert!(
        preparation::run(&shared, &parent, move |_| {
            marker.store(true, Ordering::Release);
            Ok(())
        })
        .await
        .is_err()
    );
    assert!(!called.load(Ordering::Acquire));
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn panicked_inspection_keeps_uncertain_reservation() {
    let (root, sessions, parent, _, child_id) = setup();
    let shared = Arc::new(Mutex::new(sessions));
    let context = shared.clone();
    let expected = parent.clone();
    let (send, receive) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        preparation::run(
            &context,
            &expected,
            move |flag| -> Result<(), sungeki_protocol::ApiError> {
                send.send(()).unwrap();
                await_cancel(&flag, &AtomicBool::new(false));
                panic!("simulated inspection failure");
            },
        )
        .await
    });
    receive.await.unwrap();
    build_record::request_cancel(&mut shared.lock().unwrap(), &parent).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(4), task)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    let mut sessions = shared.lock().unwrap();
    let row = sessions.store.load_job(&parent.id).unwrap().unwrap();
    assert_eq!(
        build_record::decode(&row).unwrap().1.child_id.as_deref(),
        Some(child_id.as_str())
    );
    assert!(!build_record::settle_cancel(&mut sessions, &row).unwrap());
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
