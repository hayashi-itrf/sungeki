use sungeki_project::*;

fn recipe() -> BuildRecipe {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap()
}
fn preset(content: PresetContent) -> Preset {
    Preset {
        schema_version: 1,
        name: "会話用".into(),
        description: "再利用する設定".into(),
        tags: vec!["会話".into()],
        content,
    }
}
fn character_preset() -> Preset {
    let r = recipe();
    preset(PresetContent::Character {
        character: r.roles[0].character.clone(),
        standing: Some(r.roles[0].standing.clone()),
        assets: r.assets,
    })
}

#[test]
fn voice_character_template_roundtrip_keeps_credit_and_standing() {
    let r = recipe();
    let variants = [
        preset(PresetContent::Voice {
            voice: r.roles[0].character.voice.clone(),
            credit: "VOICEVOX:ずんだもん".into(),
        }),
        character_preset(),
        preset(PresetContent::Template { recipe: r }),
    ];
    for original in variants {
        original.validate().unwrap();
        let encoded = serde_json::to_vec(&original).unwrap();
        let decoded: Preset = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, original);
        decoded.validate().unwrap();
        let mut invalid = serde_json::to_value(&original).unwrap();
        invalid["legacy_path"] = serde_json::json!("character.toml");
        assert!(serde_json::from_value::<Preset>(invalid).is_err());
    }
}

#[test]
fn invalid_voice_and_metadata_use_shared_rules() {
    let mut p = character_preset();
    p.schema_version = 2;
    assert!(p.validate().is_err());
    p.schema_version = 1;
    p.tags.push(p.tags[0].clone());
    assert!(p.validate().is_err());
    p.tags.pop();
    let PresetContent::Character { character, .. } = &mut p.content else {
        unreachable!()
    };
    let Voice::Voicevox { settings, .. } = &mut character.voice else {
        unreachable!()
    };
    settings.speed_scale = 0.;
    assert!(p.validate().is_err());
    let mut p = character_preset();
    let PresetContent::Character { character, .. } = &mut p.content else {
        unreachable!()
    };
    character.credit.clear();
    assert!(p.validate().is_err());
}

#[test]
fn cross_project_relative_paths_and_missing_standing_are_rejected() {
    let mut p = character_preset();
    let PresetContent::Character { assets, .. } = &mut p.content else {
        unreachable!()
    };
    assets[0].locator = AssetLocator::ProjectRelative {
        path: "image.png".into(),
    };
    assert!(p.validate().unwrap_err().0.contains("相対パス"));
    let mut p = character_preset();
    let PresetContent::Character { assets, .. } = &mut p.content else {
        unreachable!()
    };
    assets.clear();
    assert!(p.validate().is_err());
    let mut p = character_preset();
    let PresetContent::Character {
        assets, standing, ..
    } = &mut p.content
    else {
        unreachable!()
    };
    assets.clear();
    *standing = None;
    p.validate().unwrap(); // Voice/subtitle-only characters are useful too.
}

#[test]
fn recipe_standing_credit_rules_remain_strict_after_sharing_validation() {
    for credit in ["bad\0credit".into(), "x".repeat(65537)] {
        let mut r = recipe();
        let BuildStanding::Image { asset_id, .. } = &r.roles[0].standing else {
            unreachable!()
        };
        r.assets
            .iter_mut()
            .find(|a| &a.id == asset_id)
            .unwrap()
            .credit = credit;
        assert!(r.validate().is_err());
    }
}
