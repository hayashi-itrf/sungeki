use sungeki_editor::{Command, History};
use sungeki_project::*;

fn fixture(name: &str) -> Project {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
fn command(asset: &Asset, probe: AssetProbe) -> Command {
    Command::RelinkAsset {
        asset_id: asset.id.clone(),
        locator: AssetLocator::Absolute {
            path: "/tmp/relocated-original".into(),
        },
        probe,
    }
}
#[test]
fn every_media_kind_can_relocate_without_changing_clips_and_undo() {
    for name in [
        "audio-project.json",
        "video-clip-project.json",
        "psd-project.json",
        "vrm-clip-project.json",
        "sprite-project.json",
    ] {
        let project = fixture(name);
        assert!(!project.assets.is_empty());
        for asset in &project.assets {
            let mut history = History::new(project.clone()).unwrap();
            history
                .apply(0, &[command(asset, asset.probe.clone())])
                .unwrap();
            let mut expected = project.clone();
            expected
                .assets
                .iter_mut()
                .find(|a| a.id == asset.id)
                .unwrap()
                .locator = AssetLocator::Absolute {
                path: "/tmp/relocated-original".into(),
            };
            assert_eq!(history.project(), &expected, "{name}");
            history.undo(1).unwrap();
            assert_eq!(history.project(), &project);
        }
    }
}
#[test]
fn wrong_kind_and_vrm_role_are_atomic_even_for_unused_assets() {
    let project = fixture("vrm-assets-project.json");
    let asset = &project.assets[0];
    let mut history = History::new(project.clone()).unwrap();
    let AssetProbe::Vrm(mut probe) = asset.probe.clone() else {
        panic!()
    };
    probe.inspection.kind = match probe.inspection.kind {
        VrmKind::Model => VrmKind::Motion,
        VrmKind::Motion => VrmKind::Model,
    };
    probe.inspection.version = "1.0".into();
    probe.inspection.expressions.clear();
    let audio = fixture("audio-project.json")
        .assets
        .into_iter()
        .find(|a| matches!(a.probe, AssetProbe::Audio(_)))
        .unwrap();
    for probe in [AssetProbe::Vrm(probe), audio.probe] {
        assert!(history.apply(0, &[command(asset, probe)]).is_err());
        assert_eq!(history.project(), &project);
    }
}
#[test]
fn shorter_audio_replacement_is_rejected_without_trimming_existing_clips() {
    let project = fixture("audio-project.json");
    let asset = project
        .assets
        .iter()
        .find(|a| matches!(a.probe, AssetProbe::Audio(_)))
        .unwrap();
    let AssetProbe::Audio(mut probe) = asset.probe.clone() else {
        panic!()
    };
    probe.frames = u64::from(probe.sample_rate) / 10;
    probe.duration_us = TimeUs(100_000);
    probe.sha256 = "b".repeat(64);
    let mut history = History::new(project.clone()).unwrap();
    assert!(
        history
            .apply(0, &[command(asset, AssetProbe::Audio(probe))])
            .is_err()
    );
    assert_eq!(history.project(), &project);
}

#[test]
fn synthesized_original_can_move_but_cannot_be_replaced_with_different_audio() {
    let project = fixture("psd-speech-project.json");
    let asset = project
        .assets
        .iter()
        .find(|a| matches!(a.probe, AssetProbe::Audio(_)))
        .unwrap();
    let mut history = History::new(project.clone()).unwrap();
    history
        .apply(0, &[command(asset, asset.probe.clone())])
        .unwrap();
    assert_eq!(history.project().clips, project.clips);
    history.undo(1).unwrap();
    let AssetProbe::Audio(mut changed) = asset.probe.clone() else {
        panic!()
    };
    changed.sha256 = "c".repeat(64);
    assert!(
        history
            .apply(2, &[command(asset, AssetProbe::Audio(changed))])
            .is_err()
    );
    assert_eq!(history.project(), &project);
}

#[test]
fn ordinary_audio_can_change_content_while_retaining_timing_and_metadata() {
    let project = fixture("audio-project.json");
    let asset = project
        .assets
        .iter()
        .find(|a| matches!(a.probe, AssetProbe::Audio(_)))
        .unwrap();
    let AssetProbe::Audio(mut probe) = asset.probe.clone() else {
        panic!()
    };
    probe.sha256 = "c".repeat(64);
    let mut history = History::new(project.clone()).unwrap();
    history
        .apply(0, &[command(asset, AssetProbe::Audio(probe))])
        .unwrap();
    assert_eq!(history.project().clips, project.clips);
    assert_eq!(
        history.project().assets[0].content_hash,
        Some("c".repeat(64))
    );
    assert_eq!(history.project().assets[0].credit, asset.credit);
}
