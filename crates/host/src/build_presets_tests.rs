use super::*;
#[test]
fn bundled_standing_rejects_wrong_pose_voice_hash_credit_and_unknown_id() {
    let mut recipe: BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    let mut role = recipe.roles[0].clone();
    role.preset_id = "zundamon-sakamoto-2.3-mouth".into();
    let preset = bundled(&role.preset_id).unwrap();
    let sungeki_project::AssetProbe::Image(probe) = &mut recipe.assets[0].probe else {
        panic!()
    };
    // Only exercise preset matching here; full recipe/PSD validation is separate.
    probe.sha256 = preset.source_sha256.clone();
    recipe.assets[0].credit = preset.credit;
    role.character.credit = "VOICEVOX:ずんだもん".into();
    role.standing = BuildStanding::Psd {
        asset_id: recipe.assets[0].id.clone(),
        flip_x: false,
        pose: sungeki_project::PsdPose {
            source_sha256: preset.source_sha256,
            selections: preset.selections,
        },
        speech: sungeki_project::PsdSpeechBinding {
            character_id: role.character.id.clone(),
            group: preset.speech.group,
            closed_option: preset.speech.closed_option,
            open_option: preset.speech.open_option,
        },
    };
    validate_role(&recipe, &role).unwrap();
    let mut wrong = role.clone();
    let BuildStanding::Psd { speech, .. } = &mut wrong.standing else {
        panic!()
    };
    speech.open_option = speech.closed_option.clone();
    assert!(validate_role(&recipe, &wrong).is_err());
    let mut wrong = role.clone();
    let BuildStanding::Psd { pose, .. } = &mut wrong.standing else {
        panic!()
    };
    pose.source_sha256 = "0".repeat(64);
    assert!(validate_role(&recipe, &wrong).is_err());
    let mut wrong = role.clone();
    let BuildStanding::Psd { pose, .. } = &mut wrong.standing else {
        panic!()
    };
    pose.selections.clear();
    assert!(validate_role(&recipe, &wrong).is_err());
    let mut wrong = role.clone();
    let Voice::Voicevox { settings, .. } = &mut wrong.character.voice else {
        panic!("VOICEVOX fixture required");
    };
    settings.style_id = 2;
    assert!(validate_role(&recipe, &wrong).is_err());
    let mut wrong = role.clone();
    wrong.character.credit.clear();
    assert!(validate_role(&recipe, &wrong).is_err());
    let mut wrong = role.clone();
    wrong.preset_id = "unknown".into();
    assert!(validate_role(&recipe, &wrong).is_err());
    let mut art_only = role.clone();
    art_only.preset_id = "zundamon-sakamoto-2.3-art-mouth".into();
    art_only.character.voice = serde_json::from_value(serde_json::json!({
        "type":"irodori", "settings":{"voice_id":"none","caption":null,"cfg_scale":3.0,"num_steps":40,"seed":42},
        "identity":{"checkpoint":"test","model_manifest_sha256":"a".repeat(64),"codec_manifest_sha256":"b".repeat(64),"engine_manifest_sha256":"c".repeat(64),"voice_manifest_sha256":"d".repeat(64),"sample_rate":24000,"channels":1}
    })).unwrap();
    art_only.character.credit = "音声：Irodori-TTS".into();
    validate_role(&recipe, &art_only).unwrap();
    let mut incompatible = art_only.clone();
    incompatible.preset_id = role.preset_id.clone();
    assert!(validate_role(&recipe, &incompatible).is_err());
    let mut bad_art = art_only.clone();
    let BuildStanding::Psd { pose, .. } = &mut bad_art.standing else {
        panic!()
    };
    pose.selections.clear();
    assert!(validate_role(&recipe, &bad_art).is_err());
    recipe.assets[0].credit.clear();
    assert!(validate_role(&recipe, &art_only).is_err());
    assert!(validate_role(&recipe, &role).is_err());
    assert!(bundled("metan-sakamoto-2.1-mouth").is_ok());
    assert!(bundled("zundamon-sakamoto-2.3-static").is_err());
}

#[test]
fn custom_sprite_requires_its_own_preset_and_credit_for_every_pose() {
    let mut recipe: BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe.roles[1].preset_id = "image-custom-v1".into();
    let BuildStanding::Image { asset_id, .. } = &recipe.roles[0].standing else {
        panic!()
    };
    let id = asset_id.clone();
    recipe.roles[0].standing = BuildStanding::Sprite {
        sprites: sungeki_project::SpriteSet {
            default_pose: "通常".into(),
            poses: vec![sungeki_project::SpritePose {
                name: "通常".into(),
                closed_asset_id: id.clone(),
                open_asset_id: None,
            }],
        },
        pose: None,
        flip_x: false,
    };
    assert!(validate(&recipe).is_err());
    recipe.roles[0].preset_id = "sprite-custom-v1".into();
    validate(&recipe).unwrap();
    recipe
        .assets
        .iter_mut()
        .find(|a| a.id == id)
        .unwrap()
        .credit
        .clear();
    assert!(validate(&recipe).is_err());
}

#[test]
fn vrm_custom_preset_requires_matching_model_motion_and_speech_credit() {
    let mut r: BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    let mut p: sungeki_project::Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/vrm-clip-project.json"
    ))
    .unwrap();
    let sungeki_project::ClipContent::Vrm { mut standing } = p.clips[0].content.clone() else {
        panic!()
    };
    standing.character_id = Some(r.roles[0].character.id.clone());
    for asset in &mut p.assets {
        let previous = asset.id.clone();
        asset.id = uuid::Uuid::new_v4().to_string();
        if standing.model_asset_id == previous {
            standing.model_asset_id = asset.id.clone();
        }
        if standing.motion_asset_id == previous {
            standing.motion_asset_id = asset.id.clone();
        }
    }
    r.assets.extend(p.assets);
    r.roles[0].standing = BuildStanding::Vrm {
        standing,
        flip_x: false,
    };
    assert!(validate(&r).is_err());
    r.roles[0].preset_id = "vrm-custom-v1".into();
    r.roles[1].preset_id = "image-custom-v1".into();
    validate(&r).unwrap();
    r.assets.last_mut().unwrap().credit.clear();
    assert!(validate(&r).is_err());
}
