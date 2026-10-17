use sungeki_editor::*;
use sungeki_project::*;
fn fixture() -> Project {
    serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap()
}
fn pose(p: &Project) -> PsdPose {
    let ClipContent::Psd { pose, .. } = &p.clips[0].content else {
        panic!()
    };
    pose.clone()
}
#[test]
fn original_and_pose_roundtrip_and_undo_preserve_scene_membership() {
    let project = fixture();
    validate(&project).unwrap();
    let mut history = History::new(project.clone()).unwrap();
    let mut selected = pose(&project);
    selected.selections[0].option = "/0/0".into();
    history
        .apply(
            0,
            &[Command::SetPsd {
                clip_id: project.clips[0].id.clone(),
                asset_id: project.assets[0].id.clone(),
                pose: selected.clone(),
            }],
        )
        .unwrap();
    assert_eq!(pose(history.project()), selected);
    assert_eq!(
        history.project().clips[0].scene_id,
        project.clips[0].scene_id
    );
    let json = serde_json::to_vec(history.project()).unwrap();
    assert_eq!(
        serde_json::from_slice::<Project>(&json).unwrap(),
        *history.project()
    );
    history.undo(1).unwrap();
    assert_eq!(history.project(), &project);
}
#[test]
fn invalid_pose_and_removal_reject_the_whole_edit() {
    let project = fixture();
    for option in ["/9/0", "/0"] {
        let mut selected = pose(&project);
        selected.selections[0].option = option.into();
        let mut history = History::new(project.clone()).unwrap();
        assert!(
            history
                .apply(
                    0,
                    &[
                        Command::RenameProject {
                            title: "must not apply".into()
                        },
                        Command::SetPsd {
                            clip_id: project.clips[0].id.clone(),
                            asset_id: project.assets[0].id.clone(),
                            pose: selected
                        }
                    ]
                )
                .is_err()
        );
        assert_eq!(history.project(), &project);
    }
    assert!(
        apply_batch(
            &project,
            &[Command::RemoveAsset {
                asset_id: project.assets[0].id.clone()
            }]
        )
        .is_err()
    );
}
#[test]
fn changed_original_duplicate_selection_and_malformed_catalog_are_rejected() {
    let project = fixture();
    let AssetProbe::Psd(probe) = &project.assets[0].probe else {
        panic!()
    };
    let mut selected = pose(&project);
    selected.source_sha256 = "b".repeat(64);
    assert!(selected.validate(probe).is_err());
    let mut selected = pose(&project);
    selected.selections.push(selected.selections[0].clone());
    assert!(selected.validate(probe).is_err());
    for path in ["/00", "/4096", "/9/0", "/0/0/0"] {
        let mut probe = probe.clone();
        probe.layers[2].path = path.into();
        assert!(probe.validate().is_err());
    }
    let mut probe = probe.clone();
    probe.layers[0].visible = false;
    assert!(pose(&project).validate(&probe).is_err());
}

#[test]
fn nested_choices_resolve_parents_before_children_in_any_request_order() {
    let project = fixture();
    let AssetProbe::Psd(mut probe) = project.assets[0].probe.clone() else {
        panic!()
    };
    probe.layers[2].kind = PsdLayerKind::Group;
    for (index, visible) in [(0, true), (1, false)] {
        probe.layers.push(PsdLayer {
            path: format!("/0/1/{index}"),
            name: format!("*option{index}"),
            kind: PsdLayerKind::Pixel,
            visible,
        });
    }
    let selected = PsdPose {
        source_sha256: probe.sha256.clone(),
        selections: vec![
            PsdSelection {
                group: "/0/1".into(),
                option: "/0/1/1".into(),
            },
            PsdSelection {
                group: "/0".into(),
                option: "/0/1".into(),
            },
        ],
    };
    selected.validate(&probe).unwrap();
    let mut hidden = selected.clone();
    hidden.selections.pop();
    assert!(hidden.validate(&probe).is_err());
}
