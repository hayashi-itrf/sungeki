#![cfg(unix)]
use std::{
    ffi::OsString,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use sungeki_media::{ProcessSpec, StopReason, run_process};

fn run(
    script: &str,
    timeout: Duration,
    limit: usize,
    cancelled: &AtomicBool,
) -> std::io::Result<sungeki_media::ProcessOutput> {
    run_process(
        &ProcessSpec {
            program: Path::new("/bin/sh"),
            args: &["-c".into(), script.into()],
            directory: Path::new("/tmp"),
            timeout,
            max_output_bytes: limit,
        },
        cancelled,
    )
}

#[test]
fn captures_both_streams_and_nonzero_exit_without_a_shell_wrapper_in_runner() {
    let output = run(
        "printf stdout; printf stderr >&2; exit 7",
        Duration::from_secs(3),
        100,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output.stdout, b"stdout");
    assert_eq!(output.stderr, b"stderr");
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stopped, None);
    assert!(!output.succeeded());
    let args: Vec<OsString> = vec![
        "%s".into(),
        "literal $(touch NEVER) ; ' quotes\n日本語".into(),
    ];
    let output = run_process(
        &ProcessSpec {
            program: Path::new("/usr/bin/printf"),
            args: &args,
            directory: Path::new("/tmp"),
            timeout: Duration::from_secs(3),
            max_output_bytes: 1024,
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(output.succeeded());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        args[1].to_str().unwrap()
    );
}

#[test]
fn output_limit_bounds_memory_and_stops_a_noisy_process() {
    let start = Instant::now();
    let output = run(
        "while :; do printf 0123456789abcdef; printf fedcba9876543210 >&2; done",
        Duration::from_secs(10),
        1024,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output.stopped, Some(StopReason::OutputLimit));
    assert!(output.stdout.len() <= 1024 && output.stderr.len() <= 1024);
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn deadline_escalates_for_a_process_ignoring_term_and_settles_pipes() {
    let start = Instant::now();
    let output = run(
        "trap '' TERM; printf ready; while :; do sleep 1; done",
        Duration::from_millis(100),
        100,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output.stdout, b"ready");
    assert_eq!(output.stopped, Some(StopReason::Deadline));
    assert!(!output.succeeded());
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn cancellation_settles_owned_child_and_queued_cancel_does_not_spawn() {
    let flag = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(100));
            flag.store(true, Ordering::SeqCst);
        });
        let output = run(
            "printf ready; sleep 30",
            Duration::from_secs(60),
            100,
            &flag,
        )
        .unwrap();
        assert_eq!(output.stopped, Some(StopReason::Cancelled));
    });
    let error = run("exit 99", Duration::from_secs(1), 100, &flag).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
}

#[test]
fn normal_exit_does_not_leave_descendant_pipes_hanging() {
    let start = Instant::now();
    let output = run(
        "sleep 30 & printf done; exit 0",
        Duration::from_secs(5),
        100,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(output.succeeded());
    assert_eq!(output.stdout, b"done");
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn rejects_unbounded_or_implicit_launch_configuration() {
    let cancelled = AtomicBool::new(false);
    let mut spec = ProcessSpec {
        program: Path::new("sh"),
        args: &[],
        directory: Path::new("/tmp"),
        timeout: Duration::from_secs(1),
        max_output_bytes: 100,
    };
    assert_eq!(
        run_process(&spec, &cancelled).unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
    spec.program = Path::new("/bin/sh");
    spec.timeout = Duration::ZERO;
    assert_eq!(
        run_process(&spec, &cancelled).unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
    spec.timeout = Duration::from_secs(1);
    spec.max_output_bytes = usize::MAX;
    assert_eq!(
        run_process(&spec, &cancelled).unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
}

#[test]
fn concurrently_exiting_workers_settle_their_own_descendants() {
    // Exercise overlapping spawn/exit/group cleanup, not just an isolated child.
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                for _ in 0..16 {
                    let output = run(
                        "sleep 30 & printf done; exit 0",
                        Duration::from_secs(5),
                        100,
                        &AtomicBool::new(false),
                    )
                    .unwrap();
                    assert!(output.succeeded());
                    assert_eq!(output.stdout, b"done");
                }
            });
        }
    });
}
