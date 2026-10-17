use super::*;
use crate::speech_journal_tests::Fixture;
use std::fs;

fn until(mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !check() {
        assert!(Instant::now() < deadline, "process did not settle");
        thread::sleep(Duration::from_millis(10));
    }
}
fn start(root: &std::path::Path, exit_leader: bool) -> (OwnedProcess, u32) {
    let path = root.join("child.pid");
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            if exit_leader {
                "sleep 60 & echo $! > \"$1\"; exit 0"
            } else {
                "sleep 60 & echo $! > \"$1\"; wait"
            },
            "sbv2-process-test",
        ])
        .arg(&path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let process = OwnedProcess::spawn(command).unwrap();
    until(|| {
        fs::read_to_string(&path)
            .ok()
            .is_some_and(|s| s.trim().parse::<u32>().is_ok())
    });
    let child = fs::read_to_string(path).unwrap().trim().parse().unwrap();
    (process, child)
}
fn absent(pid: u32) -> bool {
    // SAFETY: signal zero only checks existence; no process is signalled.
    let result = unsafe { libc::kill(pid as i32, 0) };
    result == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
}
#[test]
fn stop_reaps_owned_group_without_touching_another_process() {
    let fixture = Fixture::new();
    let (mut process, child) = start(&fixture.0, false);
    let mut other = Command::new("/bin/sleep");
    other.arg("60");
    let mut other = OwnedProcess::spawn(other).unwrap();
    assert!(!other.has_exited().unwrap());
    let leader = process.id().unwrap();
    assert!(process.stop(Duration::from_secs(2)).unwrap().is_some());
    until(|| absent(child));
    assert!(absent(leader));
    assert!(!other.has_exited().unwrap());
    assert!(process.stop(Duration::from_secs(2)).unwrap().is_none());
    other.stop(Duration::from_secs(2)).unwrap();
}
#[test]
fn exited_leader_is_retained_until_its_group_is_stopped() {
    let fixture = Fixture::new();
    let (mut process, child) = start(&fixture.0, true);
    until(|| process.has_exited().unwrap());
    assert!(!absent(child));
    assert!(
        process
            .stop(Duration::from_secs(2))
            .unwrap()
            .unwrap()
            .success()
    );
    until(|| absent(child));
}
#[test]
fn dropping_owner_kills_group_and_reaps_leader() {
    let fixture = Fixture::new();
    let (process, child) = start(&fixture.0, false);
    let leader = process.id().unwrap();
    drop(process);
    until(|| absent(leader) && absent(child));
}

#[test]
fn inherited_listener_is_owned_until_process_is_stopped() {
    use std::{io::Read, net::TcpStream};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let mut command = Command::new("/usr/bin/python3");
    command.args(["-I", "-B", "-c", "import socket,time; s=socket.socket(fileno=0); c,_=s.accept(); c.sendall(b'ready'); c.close(); s.close(); time.sleep(60)"]);
    let mut process = OwnedProcess::spawn_listening(command, listener).unwrap();
    let leader = process.id().unwrap();
    let mut connection = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
    connection
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut bytes = Vec::new();
    connection.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"ready");
    assert!(TcpListener::bind(address).is_err());
    drop(connection);
    process.stop(Duration::from_secs(2)).unwrap();
    assert!(absent(leader));
    // The accepted connection may be in TIME_WAIT; check the parent's descriptor
    // is gone without relying on platform-specific TCP address-reuse behavior.
    assert!(process.listener.is_none());
}

#[test]
fn failed_spawn_releases_reserved_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture = Fixture::new();
    let command = Command::new(fixture.0.join("missing-executable"));
    assert!(OwnedProcess::spawn_listening(command, listener).is_err());
    // Other parallel tests may be between fork and exec with transient FDs.
    until(|| TcpListener::bind(address).is_ok());
}

#[test]
fn exited_group_without_surviving_children_is_reaped() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "exit 7"]);
    let mut process = OwnedProcess::spawn(command).unwrap();
    until(|| process.has_exited().unwrap());
    let pid = process.id().unwrap();
    assert_eq!(
        process
            .stop(Duration::from_secs(2))
            .unwrap()
            .unwrap()
            .code(),
        Some(7)
    );
    assert!(absent(pid));
}
