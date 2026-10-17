use std::io::{self, Read, Write};

pub(crate) struct Capture<'a> {
    pub bytes: Vec<u8>,
    written: usize,
    writer: Option<&'a mut dyn Write>,
    pub ended: bool,
}
impl<'a> Capture<'a> {
    pub fn new(writer: Option<&'a mut dyn Write>) -> Self {
        Self {
            bytes: vec![],
            written: 0,
            writer,
            ended: false,
        }
    }
    pub fn drain(&mut self, stream: &mut impl Read, limit: usize) -> io::Result<bool> {
        let mut buffer = [0_u8; 8192];
        let mut exceeded = false;
        // Bound work per poll even when a child writes continuously.
        for _ in 0..16 {
            match stream.read(&mut buffer) {
                Ok(0) => {
                    self.ended = true;
                    break;
                }
                Ok(size) => {
                    let retained = size.min(limit.saturating_sub(self.written));
                    if let Some(writer) = self.writer.as_mut() {
                        writer.write_all(&buffer[..retained])?;
                    } else {
                        self.bytes.extend_from_slice(&buffer[..retained]);
                    }
                    self.written += retained;
                    exceeded |= retained < size;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(exceeded)
    }
}

/// Wake as soon as either pipe is readable; retain the 10ms ceiling for checking
/// cancellation, deadlines and child exit when no output arrives. Negative fds
/// exclude closed pipes so their permanent HUP cannot cause a busy loop.
pub(crate) fn wait_readable(stdout: i32, stderr: i32) -> io::Result<()> {
    let mut fds = [stdout, stderr].map(|fd| libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    });
    let result = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 10) };
    if result < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
    Ok(())
}
