use super::*;
use std::time::Duration;

#[tokio::test]
async fn preempted_worker_retains_slot_until_cleanup_then_foreground_wins() {
    let slots = Arc::new(Slots::default());
    let cancelled = AtomicBool::new(false);
    let stopping = AtomicBool::new(false);
    let background = slots
        .clone()
        .acquire_background(&cancelled, &stopping)
        .await
        .unwrap();
    let foreground = tokio::spawn(slots.clone().acquire_owned());
    tokio::task::yield_now().await;
    assert!(background.interrupted.load(Ordering::Acquire));
    assert!(!foreground.is_finished());
    let waiting_background = {
        let slots = slots.clone();
        tokio::spawn(async move {
            slots
                .acquire_background(&AtomicBool::new(false), &AtomicBool::new(false))
                .await
                .unwrap()
        })
    };
    drop(background);
    let foreground = tokio::time::timeout(Duration::from_secs(1), foreground)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!waiting_background.is_finished());
    drop(foreground);
    let resumed = tokio::time::timeout(Duration::from_secs(1), waiting_background)
        .await
        .unwrap()
        .unwrap();
    assert!(!resumed.interrupted.load(Ordering::Acquire));
    assert!(!cancelled.load(Ordering::Acquire));
    drop(resumed);
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn aborted_waiter_does_not_leave_priority_reservation() {
    let slots = Arc::new(Slots::default());
    let held = slots.clone().acquire_owned().await.unwrap();
    let waiter = tokio::spawn(slots.clone().acquire_owned());
    tokio::task::yield_now().await;
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    drop(held);
    let slot = tokio::time::timeout(
        Duration::from_secs(1),
        slots
            .clone()
            .acquire_background(&AtomicBool::new(false), &AtomicBool::new(false)),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(slots.clone().try_acquire_owned().is_err());
    assert!(slot.interrupted.load(Ordering::Acquire));
    drop(slot);
    assert!(slots.try_acquire_owned().is_ok());
}

#[tokio::test]
async fn cancellation_shutdown_and_explicit_interrupt() {
    let slots = Arc::new(Slots::default());
    let yes = AtomicBool::new(true);
    let no = AtomicBool::new(false);
    assert!(slots.clone().acquire_background(&yes, &no).await.is_none());
    assert!(slots.clone().acquire_background(&no, &yes).await.is_none());
    let slot = slots.clone().acquire_background(&no, &no).await.unwrap();
    slots.interrupt_background();
    assert!(slot.interrupted.load(Ordering::Acquire));
    drop(slot);
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn background_wait_is_cancellable_without_taking_foreground_slot() {
    let slots = Arc::new(Slots::default());
    let held = slots.clone().acquire_owned().await.unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let waiting = {
        let slots = slots.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            slots
                .acquire_background(&cancel, &AtomicBool::new(false))
                .await
        })
    };
    tokio::task::yield_now().await;
    cancel.store(true, Ordering::Release);
    assert!(
        tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .unwrap()
            .unwrap()
            .is_none()
    );
    assert_eq!(slots.available_permits(), 0);
    drop(held);
    assert_eq!(slots.available_permits(), 1);
}
