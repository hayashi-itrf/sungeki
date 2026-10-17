//! Exclusive ownership of a child and its process group. No port/PID-file adoption.
//! A group signal and leader exit alone do not attest arbitrary escaped descendants.
use std::{
    io,
    net::TcpListener,
    os::{fd::OwnedFd, unix::process::CommandExt},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(crate) struct OwnedProcess {
    child: Option<Child>,
    listener: Option<TcpListener>,
}
impl OwnedProcess {
    /// Call on a blocking worker with an already checked launch command.
    pub fn spawn(command: Command) -> io::Result<Self> {
        Self::spawn_with_listener(command, None)
    }
    /// The parent reserves the endpoint before launch and keeps it until reaping.
    /// stdin is deliberately a listening socket, never a pipe or terminal.
    pub fn spawn_listening(command: Command, listener: TcpListener) -> io::Result<Self> {
        Self::spawn_with_listener(command, Some(listener))
    }
    fn spawn_with_listener(
        mut command: Command,
        listener: Option<TcpListener>,
    ) -> io::Result<Self> {
        let stdin = match &listener {
            Some(socket) => Stdio::from(OwnedFd::from(socket.try_clone()?)),
            None => Stdio::null(),
        };
        command.process_group(0).stdin(stdin);
        Ok(Self {
            child: Some(command.spawn()?),
            listener,
        })
    }
    pub fn id(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }
    /// Observe without reaping: retaining the leader reserves its PID/PGID until
    /// group shutdown. Never expose Child::try_wait to a separate observer.
    pub fn has_exited(&self) -> io::Result<bool> {
        self.child
            .as_ref()
            .map_or(Ok(true), |child| exited_without_reaping(child.id()))
    }
    /// Force-stop only our group, then reap our leader. Timeout retains ownership
    /// so callers can retry; callers must not release durable engine quarantine on timeout.
    pub fn stop(&mut self, timeout: Duration) -> io::Result<Option<ExitStatus>> {
        let Some(child) = self.child.as_mut() else {
            return Ok(None);
        };
        signal_group(child)?;
        let deadline = Instant::now() + timeout.min(Duration::from_secs(30));
        loop {
            if exited_without_reaping(child.id())? {
                let status = child.wait()?;
                self.child = None;
                self.listener = None;
                return Ok(Some(status));
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "owned SBV2 process has not exited",
                ));
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}
fn exited_without_reaping(pid: u32) -> io::Result<bool> {
    loop {
        // SAFETY: zeroed siginfo is valid output storage, pid is our unreaped child.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                pid,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            return Ok(unsafe { info.si_pid() } != 0);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}
fn signal_group(child: &Child) -> io::Result<()> {
    // ECHILD fails closed if ownership was lost; never kill a recycled PID/group.
    exited_without_reaping(child.id())?;
    let pid = i32::try_from(child.id()).map_err(|_| io::Error::other("invalid child PID"))?;
    // SAFETY: spawn created PGID=child PID; the leader has not been reaped.
    let result = unsafe { libc::kill(-pid, libc::SIGKILL) };
    if result == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    #[cfg(target_os = "macos")]
    if error.raw_os_error() == Some(libc::EPERM) && sungeki_media::owned_group_only_exited(pid)? {
        return Ok(());
    }
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Best effort on cancellation/panic. This is not a settlement receipt.
            if signal_group(&child).is_ok() {
                let listener = self.listener.take();
                let _ = thread::Builder::new()
                    .name("sbv2-reaper".into())
                    .spawn(move || {
                        let _ = child.wait();
                        drop(listener);
                    });
            }
        }
    }
}

#[cfg(test)]
mod tests;
