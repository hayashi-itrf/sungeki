use std::{ffi::CString, io, os::unix::ffi::OsStrExt, path::Path};

/// A preflight estimate, not a reservation against other disk writers.
pub fn ensure_available_space(path: &Path, required: u64) -> io::Result<()> {
    let name = CString::new(path.as_os_str().as_bytes()).map_err(io::Error::other)?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // stat is read only after statvfs successfully initializes it.
    if unsafe { libc::statvfs(name.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let stat = unsafe { stat.assume_init() };
    let available = u128::from(stat.f_bavail) * u128::from(stat.f_frsize);
    check(available, required)
}
fn check(available: u128, required: u64) -> io::Result<()> {
    if available < u128::from(required) {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            format!(
                "作業用ディスクの空き容量が不足しています（必要 {} MiB、空き {} MiB）。空き容量を増やして再実行してください",
                required.div_ceil(1048576),
                available / 1048576
            ),
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capacity_boundary_is_not_silently_accepted() {
        assert_eq!(
            check(100, 101).unwrap_err().kind(),
            io::ErrorKind::StorageFull
        );
        assert!(check(100, 100).is_ok());
        assert!(ensure_available_space(&std::env::temp_dir(), 1).is_ok());
        assert!(ensure_available_space(Path::new("/nonexistent-vvm-volume"), 1).is_err());
    }

    #[test]
    fn real_filesystem_capacity_rejects_an_impossible_reservation() {
        // Exercise statvfs and the same check used by collection, without filling
        // the disk. This does not simulate an ENOSPC during a later write.
        let error = ensure_available_space(&std::env::temp_dir(), u64::MAX).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("空き容量が不足"));
        assert!(message.contains("必要"));
        assert!(message.contains("空き"));
    }
}
