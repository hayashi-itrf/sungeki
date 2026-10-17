use crate::{sessions::Sessions, speech_journal_tests::Fixture as BaseFixture};
pub(super) struct Fixture(
    pub(super) std::path::PathBuf,
    #[allow(dead_code)] BaseFixture,
);
impl Fixture {
    pub(super) fn new() -> Self {
        let parent = BaseFixture::new();
        let engine = parent.0.join("engine");
        std::fs::create_dir(&engine).unwrap();
        Self(engine, parent)
    }
    pub(super) fn open(&self) -> Sessions {
        Sessions::open(&self.0.join("recovery.sqlite")).unwrap()
    }
}
