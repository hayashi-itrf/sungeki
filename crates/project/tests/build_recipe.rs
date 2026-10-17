use sungeki_project::*;
fn id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn fixture() -> BuildRecipe {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap()
}

#[test]
fn roundtrip_preserves_order_spoken_text_and_line_based_diagrams() {
    let mut recipe = fixture();
    recipe.validate().unwrap();
    let mut second = recipe.scenes[0].clone();
    second.name = "次の場面".into();
    second.lines.reverse();
    recipe.scenes.push(second);
    let encoded = serde_json::to_vec(&recipe).unwrap();
    let decoded: BuildRecipe = serde_json::from_slice(&encoded).unwrap();
    decoded.validate().unwrap();
    assert_eq!(decoded, recipe);
    assert_eq!(
        decoded.scenes[0].lines[0].subtitle_text,
        " 前後空白と\n改行を保持。 "
    );
    let mut json = serde_json::to_value(&recipe).unwrap();
    json["legacy_start_ms"] = serde_json::json!(100);
    assert!(serde_json::from_value::<BuildRecipe>(json).is_err());
}
#[test]
fn invalid_roles_assets_credits_and_diagram_ranges_are_rejected() {
    let original = fixture();
    let mut r = original.clone();
    r.scenes[0].lines[1].role = "unknown".into();
    let error = r.validate().unwrap_err().0;
    assert!(error.contains("シーン1") && error.contains("2行目"));
    let mut r = original.clone();
    r.roles[1].role = r.roles[0].role.clone();
    assert!(r.validate().is_err());
    let mut r = original.clone();
    r.roles[1].character.id = r.roles[0].character.id.clone();
    assert!(r.validate().is_err());
    let mut r = original.clone();
    r.roles[0].character.credit = " ".into();
    assert!(r.validate().is_err());
    let mut r = original.clone();
    r.assets[0].credit = "".into();
    assert!(r.validate().is_err());
    let mut r = original.clone();
    r.assets[0].content_hash = Some("b".repeat(64));
    assert!(r.validate().is_err());
    let mut r = original.clone();
    r.scenes[0].background_asset_id = id(999);
    assert!(r.validate().is_err());
    let mut r = original.clone();
    r.scenes[0].diagrams[0].last_line = 2;
    assert!(r.validate().is_err());
    let mut r = original.clone();
    let duplicate = r.scenes[0].diagrams[0].clone();
    r.scenes[0].diagrams.push(duplicate);
    assert!(r.validate().is_err());
    let mut r = original.clone();
    r.scenes[0].lines[0].pose = Some(PsdPose {
        source_sha256: "a".repeat(64),
        selections: vec![],
    });
    assert!(r.validate().is_err());
    let mut r = original.clone();
    r.preset_id = "unsupported".into();
    assert!(r.validate().is_err());
    let mut r = original;
    r.foreword.as_mut().unwrap().duration_us = TimeUs(1);
    assert!(r.validate().is_err());
}
#[test]
fn count_and_time_bounds_cover_whole_recipe_not_individual_scenes() {
    let mut r = fixture();
    let line = r.scenes[0].lines[0].clone();
    r.scenes[0].lines = vec![line.clone(); 200];
    r.validate().unwrap();
    r.scenes[0].lines.push(line);
    assert!(r.validate().is_err());
    let mut r = fixture();
    r.scenes[0].gap_us = TimeUs(60000001);
    assert!(r.validate().is_err());
    let mut r = fixture();
    r.scenes[0].lines.clear();
    assert!(r.validate().is_err());
    let mut r = fixture();
    r.scenes = vec![r.scenes[0].clone(); 101];
    assert!(r.validate().is_err());
}

#[test]
fn foreword_matches_editor_text_limits_before_synthesis() {
    for text in ["x".repeat(4097), "1\n2\n3\n4\n5".into()] {
        let mut r = fixture();
        r.foreword.as_mut().unwrap().text = text;
        assert!(r.validate().is_err());
    }
    let mut r = fixture();
    r.foreword.as_mut().unwrap().text = "1\n2\n3\n4".into();
    r.validate().unwrap();
}

#[test]
fn narration_fixture_requires_exactly_one_role_and_explicit_line_references() {
    let recipe: BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-narration.json"
    ))
    .unwrap();
    recipe.validate().unwrap();
    let mut invalid = recipe.clone();
    invalid.scenes[0].lines[0].role = "unknown".into();
    assert!(invalid.validate().is_err());
    invalid = recipe.clone();
    invalid.roles.clear();
    assert!(invalid.validate().is_err());
    invalid = recipe.clone();
    invalid.roles.push(recipe.roles[0].clone());
    assert!(invalid.validate().is_err());
}
