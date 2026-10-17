use sungeki_project::*;

fn asset(id: &str) -> Asset {
    Asset {
        id: id.into(),
        locator: AssetLocator::Absolute {
            path: format!("/sprites/{id}.png"),
        },
        name: id.into(),
        source: None,
        credit: "fixture".into(),
        content_hash: Some("a".repeat(64)),
        probe: AssetProbe::Image(ImageProbe {
            format: ImageFormat::Png,
            width: 640,
            height: 960,
            orientation: 1,
            has_alpha: true,
            byte_length: 100,
            sha256: "a".repeat(64),
        }),
    }
}
fn set() -> SpriteSet {
    SpriteSet {
        default_pose: "通常".into(),
        poses: vec![
            SpritePose {
                name: "通常".into(),
                closed_asset_id: "closed".into(),
                open_asset_id: Some("open".into()),
            },
            SpritePose {
                name: "笑顔".into(),
                closed_asset_id: "smile".into(),
                open_asset_id: None,
            },
        ],
    }
}
fn assets() -> Vec<Asset> {
    ["closed", "open", "smile"].into_iter().map(asset).collect()
}
#[test]
fn selects_pose_and_mouth_without_hiding_unknown_names() {
    let set = set();
    set.validate(&assets()).unwrap();
    assert_eq!(set.asset_id(None, false).unwrap(), "closed");
    assert_eq!(set.asset_id(None, true).unwrap(), "open");
    assert_eq!(set.asset_id(Some("笑顔"), true).unwrap(), "smile");
    assert!(set.asset_id(Some("笑顔2"), false).is_err());
    assert!(set.asset_id(Some(""), false).is_err());
    let encoded = serde_json::to_vec(&set).unwrap();
    assert_eq!(serde_json::from_slice::<SpriteSet>(&encoded).unwrap(), set);
    let mut value = serde_json::to_value(&set).unwrap();
    value["root_dir"] = serde_json::json!("/old/path");
    assert!(serde_json::from_value::<SpriteSet>(value).is_err());
}
#[test]
fn all_variants_are_dependencies_and_shared_images_are_deduplicated() {
    let mut set = set();
    set.poses[1].open_asset_id = Some("open".into());
    assert_eq!(set.asset_ids(), vec!["closed", "open", "smile"]);
    set.validate(&assets()).unwrap();
    // Even an unused pose must remain resolvable after collecting the project.
    assert!(set.validate(&assets()[..2]).is_err());
}
#[test]
fn rejects_bad_default_duplicate_names_and_unbounded_definitions() {
    for name in ["", " ", "bad\nname", "通常"] {
        let mut set = set();
        set.poses[1].name = name.into();
        assert!(set.validate(&assets()).is_err());
    }
    let mut set = set();
    set.default_pose = "missing".into();
    assert!(set.validate(&assets()).is_err());
    set.poses.clear();
    assert!(set.validate(&assets()).is_err());
    for n in 0..257 {
        set.poses.push(SpritePose {
            name: n.to_string(),
            closed_asset_id: "closed".into(),
            open_asset_id: None,
        });
    }
    set.default_pose = "0".into();
    assert!(set.validate(&assets()).is_err());
    set.poses.pop();
    set.validate(&assets()).unwrap();
}
#[test]
fn rejects_missing_wrong_format_orientation_size_and_hash() {
    for mutate in [
        |p: &mut ImageProbe| p.format = ImageFormat::Jpeg,
        |p: &mut ImageProbe| p.orientation = 6,
        |p: &mut ImageProbe| p.width += 1,
        |p: &mut ImageProbe| p.height = 0,
        |p: &mut ImageProbe| p.sha256 = "b".repeat(64),
        |p: &mut ImageProbe| p.byte_length = 0,
    ] {
        let mut assets = assets();
        let AssetProbe::Image(probe) = &mut assets[1].probe else {
            unreachable!()
        };
        mutate(probe);
        assert!(set().validate(&assets).is_err());
    }
    let mut assets = assets();
    assets.push(asset("closed"));
    assert!(set().validate(&assets).is_err());
}

#[test]
fn rejects_non_image_and_empty_reference_even_if_asset_exists() {
    let project: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap();
    let mut assets = assets();
    assets[1].probe = project.assets[0].probe.clone();
    assets[1].content_hash = Some(assets[1].probe.sha256().into());
    assert!(set().validate(&assets).is_err());
    let mut set = set();
    set.poses[0].closed_asset_id.clear();
    assets[0].id.clear();
    assert!(set.validate(&assets).is_err());
}
