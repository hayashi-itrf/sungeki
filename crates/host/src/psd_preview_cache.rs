//! Bounded process-local derivatives. Every hit revalidates the original bytes;
//! neither revision nor a mutable path alone proves a cached pose is current.
use crate::render_record::invalid;
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    sync::Mutex,
};
use sungeki_project::{PsdPose, PsdProbe};
use sungeki_protocol::ApiError;
#[derive(Clone)]
pub(super) struct Pixels {
    pub width: u32,
    pub height: u32,
    pub png: String,
}
#[derive(Default)]
pub(super) struct Cache {
    entries: VecDeque<([u8; 32], Pixels)>,
    bytes: usize,
}
const LIMIT: usize = 32 * 1024 * 1024;
static CACHE: Mutex<Cache> = Mutex::new(Cache {
    entries: VecDeque::new(),
    bytes: 0,
});
impl Cache {
    pub fn get(&mut self, key: &[u8; 32]) -> Option<Pixels> {
        let index = self.entries.iter().position(|(k, _)| k == key)?;
        let entry = self.entries.remove(index)?;
        let value = entry.1.clone();
        self.entries.push_back(entry);
        Some(value)
    }
    pub fn insert(&mut self, key: [u8; 32], value: Pixels) {
        if value.png.len() > LIMIT {
            return;
        }
        if let Some(index) = self.entries.iter().position(|(k, _)| *k == key) {
            self.bytes -= self.entries.remove(index).unwrap().1.png.len();
        }
        while self.bytes + value.png.len() > LIMIT || self.entries.len() >= 16 {
            if let Some((_, old)) = self.entries.pop_front() {
                self.bytes -= old.png.len();
            } else {
                break;
            }
        }
        self.bytes += value.png.len();
        self.entries.push_back((key, value));
    }
}
pub(super) fn key(
    path: &Path,
    probe: &PsdProbe,
    pose: &PsdPose,
    worker: &Path,
    python: &Path,
) -> Result<[u8; 32], ApiError> {
    verify(path, probe)?;
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(&(probe, pose, python)).map_err(invalid)?);
    let parent = worker
        .parent()
        .ok_or_else(|| invalid("PSD workerの配置が不正です"))?;
    for name in ["psd_preview.py", "psd_pose.py", "original_bytes.py"] {
        hash.update(fs::read(parent.join(name)).map_err(invalid)?);
    }
    Ok(hash.finalize().into())
}
pub(super) fn get(key: &[u8; 32]) -> Result<Option<Pixels>, ApiError> {
    Ok(CACHE.lock().map_err(invalid)?.get(key))
}
pub(super) fn insert(key: [u8; 32], value: Pixels) -> Result<(), ApiError> {
    CACHE.lock().map_err(invalid)?.insert(key, value);
    Ok(())
}
pub(super) fn verify(path: &Path, probe: &PsdProbe) -> Result<(), ApiError> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(invalid)?;
    let before = file.metadata().map_err(invalid)?;
    if !path.is_absolute()
        || !before.is_file()
        || before.len() != probe.byte_length
        || before.len() > 64 * 1024 * 1024
    {
        return Err(invalid("PSD原本の種類または容量が変わっています"));
    }
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    let mut count = 0;
    loop {
        let n = file.read(&mut buf).map_err(invalid)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > probe.byte_length {
            return Err(invalid("PSD原本が読込中に変わっています"));
        }
        hash.update(&buf[..n]);
    }
    let after = file.metadata().map_err(invalid)?;
    let current = fs::symlink_metadata(path).map_err(invalid)?;
    let stamp = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    if !current.is_file()
        || stamp(&before) != stamp(&after)
        || stamp(&after) != stamp(&current)
        || count != probe.byte_length
        || format!("{:x}", hash.finalize()) != probe.sha256
    {
        return Err(invalid("PSD原本が登録時の内容と一致しません"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_and_replacement_do_not_leak_entries() {
        let mut c = Cache::default();
        for n in 0..20 {
            c.insert(
                [n; 32],
                Pixels {
                    width: 1,
                    height: 1,
                    png: "x".repeat(3 * 1024 * 1024),
                },
            );
        }
        assert!(c.bytes <= LIMIT);
        assert!(c.entries.len() <= 16);
        assert!(c.get(&[0; 32]).is_none());
        assert!(c.get(&[19; 32]).is_some());
        c.insert(
            [19; 32],
            Pixels {
                width: 2,
                height: 1,
                png: "ok".into(),
            },
        );
        assert_eq!(c.get(&[19; 32]).unwrap().width, 2);
        assert_eq!(
            c.bytes,
            c.entries.iter().map(|(_, p)| p.png.len()).sum::<usize>()
        );
    }
    #[test]
    fn altered_deleted_and_symlinked_originals_never_hit() {
        let dir = std::env::temp_dir().join(format!("vvm-psd-cache-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let p = dir.join("source");
        fs::write(&p, b"original").unwrap();
        let probe = PsdProbe {
            sha256: format!("{:x}", Sha256::digest(b"original")),
            byte_length: 8,
            width: 1,
            height: 1,
            layer_layout_version: 1,
            layers: vec![],
        };
        assert!(verify(&p, &probe).is_ok());
        fs::write(&p, b"modified").unwrap();
        assert!(verify(&p, &probe).is_err());
        fs::remove_file(&p).unwrap();
        assert!(verify(&p, &probe).is_err());
        let other = dir.join("other");
        fs::write(&other, b"original").unwrap();
        std::os::unix::fs::symlink(&other, &p).unwrap();
        assert!(verify(&p, &probe).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
