use crate::*;
#[test]
fn custom_psd_build_and_template_require_157() {
    let mut recipe: sungeki_project::BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe.roles[0].preset_id = "psd-custom-v1".into();
    let request = SessionRequest::BuildVideo {
        request: BuildVideoRequest::Start {
            args: StartBuildVideoArgs {
                wait_ms: None,
                reuse_from: None,
                managed_profiles: Default::default(),
                job_id: "job".into(),
                recipe: recipe.clone(),
                project_path: "/tmp/custom-psd.vvm.json".into(),
                base_url: "http://127.0.0.1:50021".into(),
                engine_version: "test".into(),
            },
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 157);
    let preset = sungeki_project::Preset {
        schema_version: 1,
        name: "PSD".into(),
        description: "".into(),
        tags: vec![],
        content: sungeki_project::PresetContent::Template { recipe },
    };
    let request = SessionRequest::Presets {
        request: PresetRequest::Save {
            id: "preset".into(),
            expected_revision: 0,
            preset,
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 157);
}

#[test]
fn speech_ducking_with_entrance_fade_requires_158() {
    let mut recipe: sungeki_project::BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe.bgm = Some(serde_json::from_value(serde_json::json!({
        "asset_id":"music", "playback":{"source_start_us":0,"source_duration_us":1000000,
        "looped":true,"gain_db":-18,"volume_points":[],"fade_in_us":500000,"fade_out_us":0,"ducking":null}
    })).unwrap());
    recipe.presentation = Some(serde_json::from_value(serde_json::json!({
        "standing_offset_y":0,"credits":null,"bgm_speech_envelope":{"speech_level":0.25,"ramp_us":500000}
    })).unwrap());
    let mut request = SessionRequest::BuildVideo {
        request: BuildVideoRequest::Start {
            args: StartBuildVideoArgs {
                wait_ms: None,
                reuse_from: None,
                managed_profiles: Default::default(),
                job_id: "job".into(),
                recipe: recipe.clone(),
                project_path: "/tmp/bgm-fade.vvm.json".into(),
                base_url: "http://127.0.0.1:50021".into(),
                engine_version: "test".into(),
            },
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 158);
    if let SessionRequest::BuildVideo {
        request: BuildVideoRequest::Start { args },
    } = &mut request
    {
        args.recipe.bgm.as_mut().unwrap().playback.fade_in_us = sungeki_project::TimeUs(0);
    }
    assert_eq!(request.minimum_protocol_minor(), 152);
    let preset = sungeki_project::Preset {
        schema_version: 1,
        name: "BGM".into(),
        description: "".into(),
        tags: vec![],
        content: sungeki_project::PresetContent::Template { recipe },
    };
    assert_eq!(
        SessionRequest::Presets {
            request: PresetRequest::Save {
                id: "preset".into(),
                expected_revision: 0,
                preset
            }
        }
        .minimum_protocol_minor(),
        158
    );
}

#[test]
fn visual_fitting_build_and_template_require_159() {
    let mut recipe: sungeki_project::BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe.presentation = Some(
        serde_json::from_value(
            serde_json::json!({"standing_offset_y":0,"credits":null,"bgm_speech_envelope":null,
        "fitting":{"standing":{},"diagram":null,"opening_logo":null,"opening_standing":{}}}),
        )
        .unwrap(),
    );
    let request = SessionRequest::BuildVideo {
        request: BuildVideoRequest::Start {
            args: StartBuildVideoArgs {
                wait_ms: None,
                reuse_from: None,
                managed_profiles: Default::default(),
                job_id: "job".into(),
                recipe: recipe.clone(),
                project_path: "/tmp/fit.vvm.json".into(),
                base_url: "http://127.0.0.1:50021".into(),
                engine_version: "test".into(),
            },
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 159);
    let preset = sungeki_project::Preset {
        schema_version: 1,
        name: "Fit".into(),
        description: "".into(),
        tags: vec![],
        content: sungeki_project::PresetContent::Template { recipe },
    };
    assert_eq!(
        SessionRequest::Presets {
            request: PresetRequest::Save {
                id: "preset".into(),
                expected_revision: 0,
                preset
            }
        }
        .minimum_protocol_minor(),
        159
    );
}
