//! Darwin returns EPERM for a process group containing only zombies. Do not
//! confuse that case with a live process for which signaling was denied.
#[cfg(target_os = "macos")]
/// Caller must retain its unreaped leader to reserve the group ID.
pub fn only_exited(group: i32) -> std::io::Result<bool> {
    const PROC_PGRP_ONLY: u32 = 2; // sys/proc_info.h
    let mut pids = [0_i32; 4096];
    let capacity = std::mem::size_of_val(&pids) as i32;
    let bytes = unsafe {
        libc::proc_listpids(
            PROC_PGRP_ONLY,
            group as u32,
            pids.as_mut_ptr().cast(),
            capacity,
        )
    };
    if bytes < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if bytes >= capacity || bytes % std::mem::size_of::<i32>() as i32 != 0 {
        return Err(std::io::Error::other(
            "process group enumeration exceeded its bound",
        ));
    }
    for &pid in &pids[..bytes as usize / std::mem::size_of::<i32>()] {
        if pid <= 0 {
            continue;
        }
        let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
        let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
        let read = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast(),
                size,
            )
        };
        if read != size {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                continue;
            }
            return Err(std::io::Error::other(format!(
                "could not inspect process group member: {error}"
            )));
        }
        let info = unsafe { info.assume_init() };
        if info.pbi_pgid == group as u32 && info.pbi_status != libc::SZOMB {
            return Ok(false);
        }
    }
    Ok(true)
}
