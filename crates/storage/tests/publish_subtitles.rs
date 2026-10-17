use std::fs;
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::PathBuf;
use sungeki_storage::{PublishError, publish_subtitles};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("vvm-subtitles-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn stages(&self) -> Vec<PathBuf> {
        fs::read_dir(&self.0)
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "tmp"))
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn utf8_and_empty_subtitles_are_published_idempotently() {
    let f = Fixture::new();
    for (name, text) in [
        (
            "字幕.srt",
            "1\n00:00:00,000 --> 00:00:01,000\nずんだもん\n\n",
        ),
        ("字幕.VTT", "WEBVTT\n\n"),
        ("空.srt", ""),
    ] {
        let path = f.0.join(name);
        assert!(
            !publish_subtitles(&path, text, || false)
                .unwrap()
                .already_present
        );
        let inode = fs::metadata(&path).unwrap().ino();
        assert!(
            publish_subtitles(&path, text, || false)
                .unwrap()
                .already_present
        );
        assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
        assert_eq!(fs::read_to_string(path).unwrap(), text);
    }
    assert!(f.stages().is_empty());
}

#[test]
fn existing_different_files_directories_and_symlinks_are_preserved() {
    let f = Fixture::new();
    let path = f.0.join("字幕.srt");
    fs::write(&path, "既存字幕").unwrap();
    assert!(matches!(
        publish_subtitles(&path, "新しい字幕", || false),
        Err(PublishError::OutputExists)
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), "既存字幕");
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(matches!(
        publish_subtitles(&path, "字幕", || false),
        Err(PublishError::OutputExists)
    ));
    assert!(path.is_dir());
    fs::remove_dir(&path).unwrap();
    let source = f.0.join("source.srt");
    fs::write(&source, "字幕").unwrap();
    symlink(&source, &path).unwrap();
    assert!(matches!(
        publish_subtitles(&path, "字幕", || false),
        Err(PublishError::OutputExists)
    ));
    assert!(fs::symlink_metadata(path).unwrap().is_symlink());
    assert_eq!(fs::read_to_string(source).unwrap(), "字幕");
    assert!(f.stages().is_empty());
}

#[test]
fn cancellation_during_write_and_before_commit_removes_stage() {
    let f = Fixture::new();
    let path = f.0.join("字幕.srt");
    let text = "あ".repeat(150_000);
    for threshold in [0, 1, text.len() as u64] {
        let result = publish_subtitles(&path, &text, || {
            threshold == 0
                || f.stages()
                    .iter()
                    .any(|p| fs::metadata(p).unwrap().len() >= threshold)
        });
        assert!(matches!(result, Err(PublishError::Cancelled)));
        assert!(!path.exists());
        assert!(f.stages().is_empty());
    }
}

#[test]
fn invalid_destination_and_replaced_stage_never_publish() {
    let f = Fixture::new();
    for path in [PathBuf::from("relative.srt"), f.0.join("movie.mp4")] {
        assert!(matches!(
            publish_subtitles(&path, "字幕", || false),
            Err(PublishError::InvalidDestination)
        ));
    }
    let path = f.0.join("字幕.srt");
    let mut replaced = None;
    let result = publish_subtitles(&path, "字幕", || {
        if replaced.is_none()
            && let Some(stage) = f.stages().first()
            && fs::metadata(stage).unwrap().len() > 0
        {
            fs::rename(stage, f.0.join("detached")).unwrap();
            fs::write(stage, "他のファイル").unwrap();
            replaced = Some(stage.clone());
        }
        false
    });
    assert!(matches!(result, Err(PublishError::Changed)));
    assert!(!path.exists());
    assert_eq!(
        fs::read_to_string(replaced.unwrap()).unwrap(),
        "他のファイル"
    );
}

#[test]
fn simultaneous_different_subtitles_never_mix_or_overwrite() {
    let f = Fixture::new();
    let path = f.0.join("字幕.vtt");
    let barrier = std::sync::Barrier::new(2);
    let run = |text: &str| {
        let mut waiting = true;
        publish_subtitles(&path, text, || {
            if waiting {
                waiting = false;
                barrier.wait();
            }
            false
        })
    };
    let results = std::thread::scope(|scope| {
        let a = scope.spawn(|| run("WEBVTT\n\n左"));
        let b = scope.spawn(|| run("WEBVTT\n\n右"));
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(PublishError::OutputExists)))
            .count(),
        1
    );
    let text = fs::read_to_string(path).unwrap();
    assert!(text == "WEBVTT\n\n左" || text == "WEBVTT\n\n右");
    assert!(f.stages().is_empty());
}
