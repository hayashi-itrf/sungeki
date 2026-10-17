use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};

/// Own only the freshly created decode directory, never an existing artifact.
pub(crate) struct Temporary(PathBuf);
impl Temporary {
    pub fn new(root: &Path) -> std::io::Result<Self> {
        let path = root.join("decoded-audio");
        fs::DirBuilder::new().mode(0o700).create(&path)?;
        Ok(Self(path))
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
    pub fn finish(self) -> std::io::Result<()> {
        fs::remove_file(self.0.join("encoded.wav"))?;
        fs::remove_dir(&self.0)
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join("encoded.wav"));
        let _ = fs::remove_dir(&self.0);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_success_failure_and_existing_directory_are_distinct() {
        let root = std::env::temp_dir().join(format!("decoded-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let work = Temporary::new(&root).unwrap();
        fs::write(work.path().join("encoded.wav"), b"pcm").unwrap();
        assert!(Temporary::new(&root).is_err());
        assert_eq!(fs::read(work.path().join("encoded.wav")).unwrap(), b"pcm");
        work.finish().unwrap();
        let work = Temporary::new(&root).unwrap();
        fs::write(work.path().join("encoded.wav"), b"partial").unwrap();
        drop(work);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        fs::remove_dir(root).unwrap();
    }
}
