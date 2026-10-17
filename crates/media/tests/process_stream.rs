#![cfg(unix)]
use std::{
    io::{self, Write},
    path::Path,
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_media::{ProcessSpec, StopReason, run_process_to_writer};

#[derive(Default)]
struct Counter(usize);
impl Write for Counter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0 += b.len();
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn run(
    script: &str,
    writer: &mut dyn Write,
    limit: usize,
    timeout: Duration,
) -> io::Result<sungeki_media::ProcessOutput> {
    run_process_to_writer(
        &ProcessSpec {
            program: Path::new("/bin/sh"),
            args: &["-c".into(), script.into()],
            directory: Path::new("/tmp"),
            timeout,
            max_output_bytes: 4096,
        },
        &AtomicBool::new(false),
        writer,
        limit,
    )
}
#[test]
fn output_larger_than_memory_capture_limit_is_streamed() {
    let mut sink = Counter::default();
    let size = 65 * 1024 * 1024;
    let out = run(
        "head -c 68157440 /dev/zero; printf done >&2",
        &mut sink,
        size,
        Duration::from_secs(30),
    )
    .unwrap();
    assert!(out.succeeded());
    assert_eq!(sink.0, size);
    assert!(out.stdout.is_empty());
    assert_eq!(out.stderr, b"done");
}
#[test]
fn stdout_limit_and_deadline_remain_enforced() {
    let mut sink = Counter::default();
    let out = run(
        "head -c 1048576 /dev/zero",
        &mut sink,
        1000,
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(out.stopped, Some(StopReason::OutputLimit));
    assert_eq!(sink.0, 1000);
    let out = run("exec sleep 10", &mut sink, 1000, Duration::from_millis(30)).unwrap();
    assert_eq!(out.stopped, Some(StopReason::Deadline));
}
#[test]
fn sink_failure_is_not_a_successful_capture() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("disk full"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let err = run(
        "exec head -c 1048576 /dev/zero",
        &mut Broken,
        1048576,
        Duration::from_secs(5),
    )
    .unwrap_err();
    assert!(err.to_string().contains("disk full"));
}
