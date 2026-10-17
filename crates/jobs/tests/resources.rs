use std::sync::{Arc, Barrier};
use sungeki_jobs::{Resource, ResourceError, Resources};
fn engine() -> Resource {
    Resource::Engine("voicevox-binding".into())
}
#[test]
fn reservations_are_atomic_and_unstarted_cancellation_releases_everything() {
    let resources = Resources::default();
    let held = resources.try_acquire("one", vec![engine()]).unwrap();
    assert!(matches!(
        resources.try_acquire("two", vec![Resource::Gpu, engine()]),
        Err(ResourceError::Busy)
    ));
    let gpu = resources
        .try_acquire("gpu-user", vec![Resource::Gpu])
        .unwrap();
    drop(held);
    drop(gpu);
    let mut next = resources
        .try_acquire("two", vec![Resource::Gpu, engine()])
        .unwrap();
    next.start().unwrap();
    next.confirm_finished().unwrap();
    assert!(resources.quarantined().unwrap().is_empty());
}
#[test]
fn interrupted_running_work_quarantines_its_whole_set_until_confirmed_settled() {
    let resources = Resources::default();
    let mut lease = resources
        .try_acquire("job", vec![engine(), Resource::Gpu])
        .unwrap();
    lease.start().unwrap();
    drop(lease);
    for resource in [engine(), Resource::Gpu] {
        assert!(matches!(
            resources.try_acquire("another", vec![resource]),
            Err(ResourceError::Quarantined)
        ));
    }
    let ticket = resources.quarantined().unwrap().remove(0);
    assert_eq!(ticket.owner(), "job");
    assert_eq!(ticket.resources(), &[engine(), Resource::Gpu]);
    assert!(
        resources
            .try_acquire("independent", vec![Resource::VideoOutput])
            .is_ok()
    );
    assert_eq!(
        Resources::default().confirm_settled(&ticket),
        Err(ResourceError::WrongRegistry)
    );
    resources.confirm_settled(&ticket).unwrap();
    let mut next = resources.try_acquire("new-job", vec![engine()]).unwrap();
    next.start().unwrap();
    drop(next);
    assert_eq!(
        resources.confirm_settled(&ticket),
        Err(ResourceError::Stale)
    );
    assert!(matches!(
        resources.try_acquire("third", vec![engine()]),
        Err(ResourceError::Quarantined)
    ));
}
#[test]
fn capacity_overrides_do_not_allow_bypass_of_quarantine_or_duplicate_owners() {
    let resources = Resources::with_capacities(vec![(engine(), 2)]).unwrap();
    let mut first = resources.try_acquire("one", vec![engine()]).unwrap();
    let second = resources.try_acquire("two", vec![engine()]).unwrap();
    assert!(matches!(
        resources.try_acquire("one", vec![Resource::Gpu]),
        Err(ResourceError::Busy)
    ));
    assert!(matches!(
        resources.try_acquire("three", vec![engine()]),
        Err(ResourceError::Busy)
    ));
    first.start().unwrap();
    drop(first);
    drop(second);
    assert!(matches!(
        resources.try_acquire("three", vec![engine()]),
        Err(ResourceError::Quarantined)
    ));
}
#[test]
fn concurrent_clones_never_reserve_the_same_single_slot() {
    let resources = Resources::default();
    let start = Arc::new(Barrier::new(9));
    let finish = Arc::new(Barrier::new(9));
    let mut threads = vec![];
    for i in 0..8 {
        let (resources, start, finish) = (resources.clone(), start.clone(), finish.clone());
        threads.push(std::thread::spawn(move || {
            start.wait();
            let lease = resources.try_acquire(&format!("job-{i}"), vec![engine(), Resource::Gpu]);
            let won = lease.is_ok();
            finish.wait();
            drop(lease);
            won
        }));
    }
    start.wait();
    finish.wait();
    assert_eq!(
        threads
            .into_iter()
            .map(|t| usize::from(t.join().unwrap()))
            .sum::<usize>(),
        1
    );
    assert!(
        resources
            .try_acquire("after", vec![engine(), Resource::Gpu])
            .is_ok()
    );
}
#[test]
fn malformed_requests_and_invalid_transitions_do_not_leak_reservations() {
    assert!(Resources::with_capacities(vec![(engine(), 0)]).is_err());
    assert!(Resources::with_capacities(vec![(engine(), 1), (engine(), 2)]).is_err());
    let resources = Resources::default();
    for keys in [
        vec![],
        vec![engine(), engine()],
        vec![Resource::Engine(" ".into())],
    ] {
        assert!(matches!(
            resources.try_acquire("job", keys),
            Err(ResourceError::InvalidRequest)
        ));
    }
    let lease = resources.try_acquire("job", vec![engine()]).unwrap();
    assert_eq!(lease.confirm_finished(), Err(ResourceError::InvalidRequest));
    let mut next = resources.try_acquire("job", vec![engine()]).unwrap();
    next.start().unwrap();
    assert_eq!(next.start(), Err(ResourceError::InvalidRequest));
    next.confirm_finished().unwrap();
}

#[test]
fn recovery_blocks_reuse_until_every_persisted_owner_is_settled() {
    let resources = Resources::recover(
        vec![],
        vec![
            ("crashed-a".into(), vec![engine(), Resource::Gpu]),
            ("crashed-b".into(), vec![engine()]),
        ],
    )
    .unwrap();
    assert!(matches!(
        resources.try_acquire("new", vec![engine()]),
        Err(ResourceError::Quarantined)
    ));
    let pending = resources.quarantined().unwrap();
    resources.confirm_settled(&pending[0]).unwrap();
    assert!(matches!(
        resources.try_acquire("new", vec![engine()]),
        Err(ResourceError::Quarantined)
    ));
    assert!(resources.try_acquire("gpu", vec![Resource::Gpu]).is_ok());
    resources.confirm_settled(&pending[1]).unwrap();
    assert!(resources.try_acquire("new", vec![engine()]).is_ok());
    assert!(
        Resources::recover(
            vec![],
            vec![
                ("same".into(), vec![engine()]),
                ("same".into(), vec![Resource::Gpu])
            ]
        )
        .is_err()
    );
}

#[test]
fn panic_after_dispatch_cannot_silently_release_execution_capacity() {
    let resources = Resources::default();
    let owner = resources.clone();
    let result = std::panic::catch_unwind(move || {
        let mut lease = owner
            .try_acquire("panic-job", vec![engine(), Resource::Gpu])
            .unwrap();
        lease.start().unwrap();
        panic!("simulated worker failure");
    });
    assert!(result.is_err());
    assert_eq!(resources.quarantined().unwrap().len(), 1);
    assert!(matches!(
        resources.try_acquire("next", vec![engine()]),
        Err(ResourceError::Quarantined)
    ));
}
