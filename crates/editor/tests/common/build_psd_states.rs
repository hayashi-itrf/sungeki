use super::*;

fn with_psd_roles() -> (Project, BuildRecipe, BuildScriptIds, BuildPsdState) {
    let (p, mut r, speech) = fixture();
    let source: Project = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let mut asset = source
        .assets
        .iter()
        .find(|a| matches!(a.probe, AssetProbe::Psd(_)))
        .unwrap()
        .clone();
    asset.id = id(900);
    let ClipContent::Psd {
        pose,
        speech: Some(binding),
        ..
    } = &source
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Psd { .. }))
        .unwrap()
        .content
    else {
        panic!()
    };
    for role in &mut r.roles {
        let mut binding = binding.clone();
        binding.character_id = role.character.id.clone();
        role.standing = BuildStanding::Psd {
            asset_id: asset.id.clone(),
            pose: pose.clone(),
            speech: binding,
            flip_x: false,
        };
    }
    r.assets.push(asset);
    let mut changed = pose.clone();
    changed.selections[0].option = binding.open_option.clone();
    let mut mouth = binding.clone();
    mouth.character_id = r.roles[1].character.id.clone();
    // Different binding from the role's default, checked and persisted together.
    std::mem::swap(&mut mouth.closed_option, &mut mouth.open_option);
    (
        p,
        r,
        speech,
        BuildPsdState {
            pose: changed,
            speech: mouth,
        },
    )
}

#[test]
fn listener_change_holds_then_resets_at_scene_boundary_and_survives_save() {
    let (p, mut r, speech, state) = with_psd_roles();
    let name = r.roles[1].role.clone();
    // First line belongs to role 0; this explicitly changes the listener.
    r.scenes[0].lines[0].psd_states = Some([(name, state.clone())].into());
    let stage = synthesize(&p, &r, &speech);
    let mut ids = layout_ids();
    ids.bgm = None;
    let full = apply_batch(
        &stage,
        &plan_build_layout(&stage, &r, &speech, &ids).unwrap(),
    )
    .unwrap();
    for (scene_index, scene) in ids.scenes.iter().enumerate() {
        for pair in &scene.standing {
            let clip = full.clips.iter().find(|c| c.id == pair[1]).unwrap();
            let ClipContent::Psd {
                pose,
                speech: Some(binding),
                ..
            } = &clip.content
            else {
                panic!()
            };
            let BuildStanding::Psd {
                pose: base,
                speech: base_binding,
                ..
            } = &r.roles[1].standing
            else {
                panic!()
            };
            assert_eq!(pose, if scene_index == 0 { &state.pose } else { base });
            assert_eq!(
                binding,
                if scene_index == 0 {
                    &state.speech
                } else {
                    base_binding
                }
            );
        }
    }
    let saved: Project = serde_json::from_slice(&serde_json::to_vec(&full).unwrap()).unwrap();
    assert_eq!(saved, full);
    assert_eq!(
        serde_json::from_slice::<BuildRecipe>(&serde_json::to_vec(&r).unwrap()).unwrap(),
        r
    );
}

#[test]
fn explicit_default_restores_pose_and_binding_and_invalid_states_fail_before_synthesis() {
    let (p, mut r, speech, state) = with_psd_roles();
    let name = r.roles[1].role.clone();
    let BuildStanding::Psd {
        pose,
        speech: binding,
        ..
    } = &r.roles[1].standing
    else {
        panic!()
    };
    let base = BuildPsdState {
        pose: pose.clone(),
        speech: binding.clone(),
    };
    r.scenes[0].lines[0].psd_states = Some([(name.clone(), state.clone())].into());
    r.scenes[0].lines[1].psd_states = Some([(name.clone(), base.clone())].into());
    let stage = synthesize(&p, &r, &speech);
    let mut ids = layout_ids();
    ids.bgm = None;
    let full = apply_batch(
        &stage,
        &plan_build_layout(&stage, &r, &speech, &ids).unwrap(),
    )
    .unwrap();
    let clip = full
        .clips
        .iter()
        .find(|c| c.id == ids.scenes[0].standing[1][1])
        .unwrap();
    assert!(
        matches!(&clip.content, ClipContent::Psd { pose, speech: Some(binding), .. } if pose == &base.pose && binding == &base.speech)
    );
    for failure in ["role", "source", "character", "mouth", "duplicate"] {
        let mut bad = r.clone();
        let states = bad.scenes[0].lines[0].psd_states.as_mut().unwrap();
        match failure {
            "role" => {
                states.insert("unknown".into(), state.clone());
            }
            "source" => states.get_mut(&name).unwrap().pose.source_sha256 = "0".repeat(64),
            "character" => {
                states.get_mut(&name).unwrap().speech.character_id =
                    bad.roles[0].character.id.clone()
            }
            "mouth" => states.get_mut(&name).unwrap().speech.open_option = "/999".into(),
            "duplicate" => {
                bad.scenes[0].lines[0].role = name.clone();
                bad.scenes[0].lines[0].pose = Some(base.pose.clone());
            }
            _ => unreachable!(),
        }
        assert!(bad.validate().is_err(), "{failure}");
    }
}
