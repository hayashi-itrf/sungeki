use crate::{
    saved_audio::prepare,
    speech_journal_tests::{Fixture, id},
    speech_output_tests::response,
};
use std::{
    fs,
    os::unix::fs::{MetadataExt, symlink},
};
use sungeki_project::{Asset, AssetLocator, AssetProbe, AudioProbe, TimeUs};
use sungeki_protocol::ErrorCode;
fn setup() -> (Fixture, Asset) {
    let fixture = Fixture::new();
    let source = fixture.0.join("original.wav");
    let wav = response().verify().unwrap().audio;
    fs::write(&source, wav.bytes()).unwrap();
    let info = wav.info();
    let asset = Asset {
        id: id(),
        name: "音声".into(),
        source: None,
        credit: "検証".into(),
        locator: AssetLocator::Absolute {
            path: source.to_str().unwrap().into(),
        },
        content_hash: Some(info.content_hash.clone()),
        probe: AssetProbe::Audio(AudioProbe {
            encoding: None,
            sha256: info.content_hash.clone(),
            duration_us: TimeUs(info.duration_us),
            sample_rate: info.sample_rate,
            channels: info.channels,
            frames: info.frames,
            byte_length: wav.bytes().len() as u64,
        }),
    };
    (fixture, asset)
}
fn source(asset: &Asset) -> &std::path::Path {
    let AssetLocator::Absolute { path } = &asset.locator else {
        panic!()
    };
    std::path::Path::new(path)
}
#[test]
fn copies_originals_without_links_and_deduplicates_content_across_asset_ids() {
    let (fixture, asset) = setup();
    let document = fixture.0.join("音声作品.vvm.json");
    let mut second = asset.clone();
    second.id = id();
    let copies = prepare(&[asset.clone(), second.clone()], &document, 92).unwrap();
    assert_eq!(copies.len(), 2);
    assert_eq!(copies[0].relocation.asset_id, asset.id);
    assert_eq!(copies[1].relocation.asset_id, second.id);
    assert_eq!(copies[0].relocation.source, asset.locator);
    assert_eq!(copies[0].destination, copies[1].destination);
    assert_eq!(
        fixture
            .0
            .canonicalize()
            .unwrap()
            .join(&copies[0].relative_path),
        copies[0].destination
    );
    assert!(
        copies[0]
            .relative_path
            .starts_with("音声作品.assets/audio/")
    );
    assert_ne!(
        fs::metadata(source(&asset)).unwrap().ino(),
        fs::metadata(&copies[0].destination).unwrap().ino()
    );
    assert_eq!(
        fs::read(&copies[0].destination).unwrap(),
        response().unverified_wav
    );
    let inode = fs::metadata(&copies[0].destination).unwrap().ino();
    prepare(&[asset.clone()], &document, 92).unwrap();
    assert_eq!(fs::metadata(&copies[0].destination).unwrap().ino(), inode);
    let mut project = sungeki_project::Project::empty(id(), "コピーと編集の接続".into());
    project.assets = vec![asset.clone(), second];
    let map = sungeki_editor::AudioRelocations::new(
        copies.iter().map(|c| c.relocation.clone()).collect(),
    )
    .unwrap();
    let relocated = map.relocate_project(&project).unwrap();
    assert_eq!(
        relocated.assets[0].locator,
        copies[0].relocation.destination
    );
    assert_eq!(
        relocated.assets[1].locator,
        copies[1].relocation.destination
    );
    assert_eq!(map.relocate_project(&relocated).unwrap(), relocated);
    assert!(!document.exists()); // Preparation never publishes the JSON document.
    fs::write(source(&asset), b"modified after capture").unwrap();
    assert_eq!(
        fs::read(&copies[0].destination).unwrap(),
        response().unverified_wav
    );
    assert_eq!(
        prepare(&[asset], &document, 92).unwrap_err().code,
        ErrorCode::ExternalModification
    );
}
#[test]
fn existing_conflicting_file_is_never_replaced() {
    let (fixture, asset) = setup();
    let document = fixture.0.join("copy.vvm.json");
    let copies = prepare(&[asset.clone()], &document, 92).unwrap();
    fs::write(&copies[0].destination, b"keep this conflicting file").unwrap();
    assert_eq!(
        prepare(&[asset.clone()], &document, 92).unwrap_err().code,
        ErrorCode::ExternalModification
    );
    assert_eq!(
        fs::read(&copies[0].destination).unwrap(),
        b"keep this conflicting file"
    );
    assert_eq!(fs::read(source(&asset)).unwrap(), response().unverified_wav);
}
#[test]
fn rejects_directory_and_destination_symlinks_without_writing_through_them() {
    for level in ["root", "audio", "file"] {
        let (fixture, asset) = setup();
        let document = fixture.0.join("copy.vvm.json");
        let elsewhere = fixture.0.join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        let root = fixture.0.join("copy.assets");
        if level == "root" {
            symlink(&elsewhere, &root).unwrap();
        } else {
            fs::create_dir(&root).unwrap();
            if level == "audio" {
                symlink(&elsewhere, root.join("audio")).unwrap();
            } else {
                fs::create_dir(root.join("audio")).unwrap();
                symlink(
                    source(&asset),
                    root.join("audio")
                        .join(format!("{}.wav", asset.content_hash.as_ref().unwrap())),
                )
                .unwrap();
            }
        }
        assert!(prepare(&[asset.clone()], &document, 92).is_err());
        assert_eq!(fs::read_dir(elsewhere).unwrap().count(), 0);
        assert_eq!(fs::read(source(&asset)).unwrap(), response().unverified_wav);
    }
}
#[test]
fn budget_and_metadata_fail_before_creating_material_directory() {
    let (fixture, asset) = setup();
    let document = fixture.0.join("copy.vvm.json");
    assert!(prepare(&[asset.clone()], &document, 91).is_err());
    let mut invalid = asset.clone();
    invalid.content_hash = Some("b".repeat(64));
    assert!(prepare(&[invalid], &document, 1000).is_err());
    assert!(prepare(&[asset], &fixture.0.join(".vvm.json"), 1000).is_err());
    assert!(!fixture.0.join("copy.assets").exists());
    assert!(prepare(&[], &document, 0).unwrap().is_empty());
    assert!(!fixture.0.join("copy.assets").exists());
}
