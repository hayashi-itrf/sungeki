use crate::{
    speech_artifacts,
    speech_journal_tests::{Fixture, id},
    speech_original::load,
    speech_output_tests::response,
};
use std::{fs, os::unix::fs::symlink};
use sungeki_protocol::ErrorCode;

#[test]
fn generated_original_is_rechecked_and_returned_as_owned_bytes() {
    let fixture = Fixture::new();
    let sessions = fixture.open();
    let artifact = speech_artifacts::store(
        sessions.speech_directory.as_ref().unwrap(),
        &id(),
        response(),
        Default::default(),
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    let checked = load(&artifact).unwrap();
    assert_eq!(checked.bytes(), response().unverified_wav);
    assert_eq!(checked.info().content_hash, artifact.sha256);
    // Already verified bytes stay stable even if the on-disk original changes.
    let mut modified = response().unverified_wav;
    *modified.last_mut().unwrap() = 1;
    fs::write(&artifact.path, modified).unwrap();
    assert_eq!(checked.bytes(), response().unverified_wav);
    assert_eq!(
        load(&artifact).unwrap_err().code,
        ErrorCode::ExternalModification
    );
}
#[test]
fn missing_truncated_oversized_and_same_length_modified_audio_are_not_accepted() {
    for change in ["missing", "truncated", "oversized", "sample", "metadata"] {
        let fixture = Fixture::new();
        let sessions = fixture.open();
        let mut artifact = speech_artifacts::store(
            sessions.speech_directory.as_ref().unwrap(),
            &id(),
            response(),
            Default::default(),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        match change {
            "missing" => fs::remove_file(&artifact.path).unwrap(),
            "truncated" => fs::write(&artifact.path, b"RIFF").unwrap(),
            "oversized" => fs::OpenOptions::new()
                .write(true)
                .open(&artifact.path)
                .unwrap()
                .set_len(sungeki_media::MAX_WAV_BYTES as u64 + 1)
                .unwrap(),
            "sample" => {
                let mut bytes = response().unverified_wav;
                bytes[44] = 1;
                fs::write(&artifact.path, bytes).unwrap();
            }
            "metadata" => {
                artifact.frames = 48;
                artifact.duration_us = 2000;
                artifact.lip_sync.as_mut().unwrap().source_frames = 48;
            }
            _ => unreachable!(),
        }
        assert_eq!(
            load(&artifact).unwrap_err().code,
            if change == "missing" {
                ErrorCode::AssetMissing
            } else {
                ErrorCode::ExternalModification
            }
        );
    }
}
#[test]
fn symlinks_directories_and_fifo_are_rejected_without_following_or_waiting() {
    for kind in ["symlink", "directory", "fifo"] {
        let fixture = Fixture::new();
        let sessions = fixture.open();
        let artifact = speech_artifacts::store(
            sessions.speech_directory.as_ref().unwrap(),
            &id(),
            response(),
            Default::default(),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        let original = std::path::Path::new(&artifact.path).with_file_name("original.wav");
        fs::rename(&artifact.path, &original).unwrap();
        match kind {
            "symlink" => symlink(&original, &artifact.path).unwrap(),
            "directory" => fs::create_dir(&artifact.path).unwrap(),
            "fifo" => {
                let path = std::ffi::CString::new(artifact.path.as_bytes()).unwrap();
                // SAFETY: path is a valid NUL-terminated test-only pathname.
                assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            load(&artifact).unwrap_err().code,
            ErrorCode::ExternalModification
        );
        assert_eq!(fs::read(original).unwrap(), response().unverified_wav);
    }
}

#[test]
fn cancelled_analysis_never_creates_an_original_directory() {
    let fixture = Fixture::new();
    let sessions = fixture.open();
    let root = sessions.speech_directory.as_ref().unwrap();
    let job_id = id();
    let error = speech_artifacts::store(
        root,
        &job_id,
        response(),
        Default::default(),
        &std::sync::atomic::AtomicBool::new(true),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Cancelled);
    assert!(!root.join(job_id).exists());
}
