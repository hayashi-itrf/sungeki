//! Blocking Unix runner for an owned media subprocess. Invoke off the GUI/IPC
//! thread. Worker descendants must remain in the process group (no daemonizing).
use crate::process_capture::{Capture, wait_readable};
use std::{
    ffi::OsString,
    io::{self, Write},
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub struct ProcessSpec<'a> {
    pub program: &'a Path,
    pub args: &'a [OsString],
    pub directory: &'a Path,
    pub timeout: Duration,
    /// Bound each stream separately; excess output triggers cancellation.
    pub max_output_bytes: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Cancelled,
    Deadline,
    OutputLimit,
}
#[derive(Debug)]
pub struct ProcessOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stopped: Option<StopReason>,
}
impl ProcessOutput {
    pub fn succeeded(&self) -> bool {
        self.status.success() && self.stopped.is_none()
    }
}

struct OwnedChild {
    child: Child,
    group: i32,
    reaped: bool,
}
impl OwnedChild {
    fn signal(&self, signal: i32) -> io::Result<()> {
        // The unreaped leader reserves the PID/group identifier until cleanup.
        if unsafe { libc::kill(-self.group, signal) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        #[cfg(target_os = "macos")]
        if error.raw_os_error() == Some(libc::EPERM)
            && crate::process_group::only_exited(self.group)?
        {
            return Ok(());
        }
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(io::Error::new(
                error.kind(),
                format!("signal process group: {error}"),
            ))
        }
    }
    fn exited(&mut self) -> io::Result<bool> {
        loop {
            let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
            // WNOWAIT observes exit without releasing the leader's identity.
            let status = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.group as libc::id_t,
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if status == 0 {
                return Ok(unsafe { info.assume_init().si_pid() } != 0);
            }
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ECHILD) {
                self.reaped = true; // Identity was released by an external reaper; never signal it.
            }
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(io::Error::new(
                    error.kind(),
                    format!("observe child exit: {error}"),
                ));
            }
        }
    }
    fn finish(&mut self) -> io::Result<ExitStatus> {
        // Also settle ordinary children left behind by a normally exited worker.
        // Never send a signal using this numeric group after wait() has reaped it.
        self.signal(libc::SIGKILL)?;
        let result = self.child.wait();
        if result.is_ok()
            || result
                .as_ref()
                .is_err_and(|e| e.raw_os_error() == Some(libc::ECHILD))
        {
            self.reaped = true;
        }
        result
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.signal(libc::SIGKILL);
            let _ = self.child.wait();
        }
    }
}

fn nonblocking(stream: &impl AsRawFd) -> io::Result<()> {
    let descriptor = stream.as_raw_fd();
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn run_process(spec: &ProcessSpec<'_>, cancelled: &AtomicBool) -> io::Result<ProcessOutput> {
    run(spec, cancelled, None, spec.max_output_bytes)
}

/// Stream stdout without retaining it in memory. The caller owns the sink and
/// removes partial files on failure. stderr still uses spec.max_output_bytes.
/// Returned stdout is empty; stopped/nonzero status must be checked before use.
pub fn run_process_to_writer(
    spec: &ProcessSpec<'_>,
    cancelled: &AtomicBool,
    writer: &mut dyn Write,
    max_stdout_bytes: usize,
) -> io::Result<ProcessOutput> {
    if max_stdout_bytes == 0 || max_stdout_bytes as u64 > 4 * 1024 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid streamed output limit",
        ));
    }
    run(spec, cancelled, Some(writer), max_stdout_bytes)
}

fn run(
    spec: &ProcessSpec<'_>,
    cancelled: &AtomicBool,
    writer: Option<&mut dyn Write>,
    stdout_limit: usize,
) -> io::Result<ProcessOutput> {
    if !spec.program.is_absolute()
        || !spec.directory.is_absolute()
        || spec.timeout.is_zero()
        || spec.timeout > Duration::from_secs(86400)
        || spec.max_output_bytes > crate::MAX_WAV_BYTES
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid media process specification",
        ));
    }
    if cancelled.load(Ordering::SeqCst) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "cancelled before process start",
        ));
    }
    let child = Command::new(spec.program)
        .args(spec.args)
        .current_dir(spec.directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let mut owner = OwnedChild {
        group: child.id() as i32,
        child,
        reaped: false,
    };
    let mut stdout = owner.child.stdout.take().unwrap();
    let mut stderr = owner.child.stderr.take().unwrap();
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let mut out = Capture::new(writer);
    let mut err = Capture::new(None);
    let deadline = Instant::now() + spec.timeout;
    let mut stop = None;
    let mut terminate_at = None;
    loop {
        let overflow = out.drain(&mut stdout, stdout_limit)?
            | err.drain(&mut stderr, spec.max_output_bytes)?;
        let exited = owner.exited()?;
        let reason = if cancelled.load(Ordering::SeqCst) {
            Some(StopReason::Cancelled)
        } else if overflow {
            Some(StopReason::OutputLimit)
        } else if Instant::now() >= deadline {
            Some(StopReason::Deadline)
        } else {
            None
        };
        if stop.is_none() && reason.is_some() {
            stop = reason;
            owner.signal(libc::SIGTERM)?;
            terminate_at = Some(Instant::now() + Duration::from_secs(2));
        }
        if exited {
            break;
        }
        if terminate_at.is_some_and(|at| Instant::now() >= at) {
            owner.signal(libc::SIGKILL)?;
            // Keep ownership while waiting for the OS to settle the process.
            terminate_at = None;
        }
        wait_readable(
            if out.ended { -1 } else { stdout.as_raw_fd() },
            if err.ended { -1 } else { stderr.as_raw_fd() },
        )?;
    }
    // Keep the exited leader unreaped until descendants release both pipes.
    // This reserves the group ID while termination is retried: a descendant may
    // still be completing fork/exec when the first group signal is delivered.
    owner.signal(libc::SIGKILL)?;
    let drain_deadline = Instant::now() + Duration::from_secs(2);
    while !out.ended || !err.ended {
        let overflow = out.drain(&mut stdout, stdout_limit)?
            | err.drain(&mut stderr, spec.max_output_bytes)?;
        if overflow && stop.is_none() {
            stop = Some(StopReason::OutputLimit);
        }
        if out.ended && err.ended {
            break;
        }
        if Instant::now() >= drain_deadline {
            return Err(io::Error::other(
                "worker descendants retained output pipes after termination",
            ));
        }
        owner.signal(libc::SIGKILL)?;
        // Cleanup retries deliberately yield so concurrent fork/exec and exit
        // can settle before the next group signal; only live transfer is polled.
        std::thread::sleep(Duration::from_millis(10));
    }
    let status = owner.finish()?;
    Ok(ProcessOutput {
        status,
        stdout: out.bytes,
        stderr: err.bytes,
        stopped: stop,
    })
}
