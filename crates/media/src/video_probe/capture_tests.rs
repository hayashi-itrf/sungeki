use super::*;
use std::{cell::Cell, time::Duration};
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn stable_snapshot_is_independent_and_partial_capture_is_removed() {
    let root = Root(std::env::temp_dir().join(format!("video-capture-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&root.0).unwrap();
    let source = root.0.join("original");
    fs::write(&source, b"old bytes").unwrap();
    let runtime = ProbeRuntime {
        ffprobe: Path::new("/unused"),
        root: &root.0,
        timeout: Duration::from_secs(3),
        max_source_bytes: 1024,
    };
    let snapshot = Snapshot::new(&source, &runtime, || Ok(())).unwrap();
    fs::write(&source, b"new bytes").unwrap();
    assert_eq!(fs::read(&snapshot.path).unwrap(), b"old bytes");
    assert_eq!(
        snapshot.sha256,
        format!("{:x}", Sha256::digest(b"old bytes"))
    );
    let work = snapshot.root.clone();
    drop(snapshot);
    assert!(!work.exists());
    let count = Cell::new(0);
    let result = Snapshot::new(&source, &runtime, || {
        count.set(count.get() + 1);
        if count.get() == 2 {
            fs::write(&source, b"changed!!").unwrap();
        }
        Ok(())
    });
    assert!(matches!(result, Err(ProbeError::Changed)));
    let count = Cell::new(0);
    let result = Snapshot::new(&source, &runtime, || {
        count.set(count.get() + 1);
        if count.get() == 2 {
            return Err(ProbeError::Stopped(crate::StopReason::Cancelled));
        }
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProbeError::Stopped(crate::StopReason::Cancelled))
    ));
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
}
