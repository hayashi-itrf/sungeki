use super::*;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn preparation_reprobes_png_and_wav_and_rejects_forged_metadata_and_changes() {
    let (root, shared, parent) = super::layout_apply_tests::ready().await;
    let mut recipe = decode(&parent).unwrap().0.recipe;
    let sessions = shared.lock().unwrap();
    let (_, current) = sessions.load("session").unwrap();
    recipe.assets = current.history().project().assets.clone();
    let before = sessions.store.load("session").unwrap().unwrap();
    drop(sessions);
    assert_eq!(
        crate::build_record::verify_sources(&shared, &recipe, || false).unwrap(),
        3
    );
    let mut forged = recipe.clone();
    let sungeki_project::AssetProbe::Image(p) = &mut forged.assets[0].probe else {
        panic!()
    };
    p.width += 1;
    assert!(crate::build_record::verify_sources(&shared, &forged, || false).is_err());
    assert!(crate::build_record::verify_sources(&shared, &recipe, || true).is_err());
    let sungeki_project::AssetLocator::Absolute { path } = &recipe.assets[0].locator else {
        panic!()
    };
    let original = std::fs::read(path).unwrap();
    std::fs::write(path, vec![0; original.len()]).unwrap();
    assert!(crate::build_record::verify_sources(&shared, &recipe, || false).is_err());
    std::fs::remove_file(path).unwrap();
    assert!(crate::build_record::verify_sources(&shared, &recipe, || false).is_err());
    let replacement = root.join("replacement.png");
    std::fs::write(&replacement, original).unwrap();
    std::os::unix::fs::symlink(&replacement, path).unwrap();
    assert!(crate::build_record::verify_sources(&shared, &recipe, || false).is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load("session")
            .unwrap()
            .unwrap(),
        before
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        parent
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn oversized_recipe_is_rejected_before_reading_sources() {
    let (root, sessions, parent) = super::reservation_tests::setup(false);
    let mut recipe = decode(&parent).unwrap().0.recipe;
    let sungeki_project::AssetProbe::Image(p) = &mut recipe.assets[0].probe else {
        panic!()
    };
    p.byte_length = 64 * 1024 * 1024 + 1;
    let shared = Arc::new(Mutex::new(sessions));
    assert!(crate::build_record::verify_sources(&shared, &recipe, || false).is_err());
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires real FFmpeg/FFprobe"]
async fn preparation_reprobes_real_video_without_mutating_project_or_parent() {
    use sungeki_project::*;
    let (root, shared, parent) = super::layout_apply_tests::ready().await;
    let sungeki_protocol::SessionResponse::RuntimeSettings { settings: expected } =
        crate::runtime_settings::execute(&shared, None).unwrap()
    else {
        panic!("runtime settings response")
    };
    let mut settings = expected.clone();
    settings.ffprobe = Some(std::env::var_os("VVM_TEST_FFPROBE").unwrap().into());
    crate::runtime_settings::execute(
        &shared,
        Some(sungeki_protocol::SaveRuntimeSettingsArgs { expected, settings }),
    )
    .unwrap();
    let mut recipe = decode(&parent).unwrap().0.recipe;
    let before = {
        let sessions = shared.lock().unwrap();
        let (_, current) = sessions.load("session").unwrap();
        recipe.assets = current.history().project().assets.clone();
        sessions.store.load("session").unwrap().unwrap()
    };
    let source = root.join("video.mp4");
    let result = std::process::Command::new(std::env::var_os("VVM_TEST_FFMPEG").unwrap())
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x120:rate=10:duration=1",
            "-c:v",
            "libx264",
            "-an",
        ])
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let original = std::fs::read(&source).unwrap();
    let sungeki_protocol::SessionResponse::VideoProbed { probe } =
        crate::image_probe::video::execute(&shared, source.to_string_lossy().into()).unwrap()
    else {
        panic!()
    };
    let asset = Asset {
        id: uuid::Uuid::new_v4().to_string(),
        name: "実動画検査".into(),
        credit: "自作testsrc2".into(),
        source: None,
        content_hash: Some(probe.sha256.clone()),
        locator: AssetLocator::Absolute {
            path: source.to_string_lossy().into(),
        },
        probe: AssetProbe::Video(probe),
    };
    recipe.scenes[0].diagrams = vec![BuildDiagram {
        fade: None,
        motion: None,
        reaction: None,
        asset_id: asset.id.clone(),
        first_line: 0,
        last_line: 0,
        video_playback: Some(VideoPlayback {
            source_start_us: TimeUs(200000),
            source_duration_us: TimeUs(500000),
            looped: true,
            audio_stream: None,
            gain_db: 0.,
        }),
    }];
    recipe.assets.push(asset);
    assert_eq!(
        crate::build_record::verify_sources(&shared, &recipe, || false).unwrap(),
        4
    );
    let mut forged = recipe.clone();
    let AssetProbe::Video(p) = &mut forged.assets.last_mut().unwrap().probe else {
        panic!()
    };
    p.metadata.video.width += 2;
    assert_eq!(
        crate::build_record::verify_sources(&shared, &forged, || false)
            .unwrap_err()
            .code,
        sungeki_protocol::ErrorCode::ExternalModification
    );
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert!(crate::build_record::verify_sources(&shared, &recipe, || true).is_err());
    std::fs::remove_file(&source).unwrap();
    assert_eq!(
        crate::build_record::verify_sources(&shared, &recipe, || false)
            .unwrap_err()
            .code,
        sungeki_protocol::ErrorCode::AssetMissing
    );
    let sessions = shared.lock().unwrap();
    let staging = crate::render_location::root(&sessions.review_database_path).unwrap();
    assert_eq!(std::fs::read_dir(staging).unwrap().count(), 0);
    assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
    assert_eq!(
        sessions.store.load_job(&parent.id).unwrap().unwrap(),
        parent
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
