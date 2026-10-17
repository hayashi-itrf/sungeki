#[test]
fn reaction_recipe_requires_108_and_absent_effect_preserves_existing_wire() {
    let mut recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    let request = |recipe| -> crate::SessionRequest {
        serde_json::from_value(serde_json::json!({"type":"build_video","request":{"type":"start","args":{
            "job_id":"j", "recipe":recipe, "project_path":"/p.vvm.json", "base_url":"http://localhost:50021", "engine_version":"test"
        }}})).unwrap()
    };
    let original = request(recipe.clone());
    assert!(original.minimum_protocol_minor() < 108);
    let roundtrip = serde_json::to_value(original).unwrap();
    assert!(
        roundtrip["request"]["args"]["recipe"]["scenes"][0]["diagrams"][0]
            .get("reaction")
            .is_none()
    );
    recipe["scenes"][0]["diagrams"][0]["reaction"] = serde_json::json!({
        "center_x":900.,"center_y":300.,"width":200.,"height":120.,"start_scale":0.5,
        "fade":{"in_us":100000,"out_us":100000}
    });
    assert_eq!(request(recipe).minimum_protocol_minor(), 108);
}

#[test]
fn scene_fade_recipe_requires_112() {
    let mut recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe["scenes"][0]["fade"] = serde_json::json!({"in_us":100000,"out_us":200000});
    let request: crate::SessionRequest = serde_json::from_value(serde_json::json!({"type":"build_video","request":{"type":"start","args":{
        "job_id":"j", "recipe":recipe, "project_path":"/p.vvm.json", "base_url":"http://localhost:50021", "engine_version":"test"
    }}})).unwrap();
    assert_eq!(request.minimum_protocol_minor(), 112);
}

#[test]
fn diagram_motion_recipe_requires_113() {
    let mut recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe["scenes"][0]["diagrams"][0]["motion"] =
        serde_json::json!({"end_x":480.,"end_y":180.,"end_scale":0.8,"easing":"linear"});
    let request: crate::SessionRequest = serde_json::from_value(serde_json::json!({"type":"build_video","request":{"type":"start","args":{
        "job_id":"j", "recipe":recipe, "project_path":"/p.vvm.json", "base_url":"http://localhost:50021", "engine_version":"test"
    }}})).unwrap();
    assert_eq!(request.minimum_protocol_minor(), 113);
}

#[test]
fn diagram_fade_recipe_requires_114() {
    let mut recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe["scenes"][0]["diagrams"][0]["fade"] =
        serde_json::json!({"in_us":100000,"out_us":200000});
    let request: crate::SessionRequest = serde_json::from_value(serde_json::json!({"type":"build_video","request":{"type":"start","args":{
        "job_id":"j", "recipe":recipe, "project_path":"/p.vvm.json", "base_url":"http://localhost:50021", "engine_version":"test"
    }}})).unwrap();
    assert_eq!(request.minimum_protocol_minor(), 114);
}

#[test]
fn bgm_reference_duration_requires_115() {
    let mut recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe["bgm"] = serde_json::json!({"asset_id":"test", "volume_point_reference_duration_us":1000000,
        "playback":{"source_start_us":0,"source_duration_us":1000000,"looped":true,"gain_db":0.,"volume_points":[],"fade_in_us":0,"fade_out_us":0,"ducking":null}});
    let make = |r| {
        serde_json::from_value::<crate::SessionRequest>(serde_json::json!({"type":"build_video","request":{"type":"start","args":{
        "job_id":"j", "recipe":r, "project_path":"/p.vvm.json", "base_url":"http://localhost:50021", "engine_version":"test"
    }}})).unwrap()
    };
    assert_eq!(make(recipe.clone()).minimum_protocol_minor(), 115);
    recipe["bgm"]
        .as_object_mut()
        .unwrap()
        .remove("volume_point_reference_duration_us");
    assert!(make(recipe).minimum_protocol_minor() < 115);
}

#[test]
fn runtime_settings_with_or_without_font_hash_require_current_schema() {
    let settings = crate::RuntimeSettings {
        format_version: 1,
        font: Some("/font.ttf".into()),
        ..Default::default()
    };
    let request = |settings| crate::SessionRequest::SaveRuntimeSettings {
        args: crate::SaveRuntimeSettingsArgs {
            expected: Default::default(),
            settings,
        },
    };
    assert_eq!(request(settings.clone()).minimum_protocol_minor(), 145);
    assert_eq!(
        request(crate::RuntimeSettings {
            font_sha256: Some("a".repeat(64)),
            ..settings
        })
        .minimum_protocol_minor(),
        145
    );
    assert_eq!(
        crate::SessionRequest::GetRuntimeSettings.minimum_protocol_minor(),
        145
    );
}
