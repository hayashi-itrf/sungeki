use serde_json::json;
use sungeki_protocol::*;

#[test]
fn build_and_saved_template_require_minor_152() {
    let mut recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe["presentation"] = json!({
        "standing_offset_y":560, "credits":null, "bgm_speech_envelope":null
    });
    for request in [
        json!({"type":"build_video","request":{"type":"start","args":{
            "job_id":"job","project_path":"/tmp/project.vvm.json","recipe":recipe,"wait_ms":0
        }}}),
        json!({"type":"presets","request":{"action":"save","id":"template","expected_revision":0,
            "preset":{"schema_version":1,"name":"test","description":"","tags":[],
                "content":{"kind":"template","recipe":recipe}}}}),
    ] {
        let request: SessionRequest = serde_json::from_value(request).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 152);
        for field in [
            (
                "slots",
                json!({"standing":{}, "subtitle":null, "diagram":null, "diagrams_behind_standings":true}),
            ),
            (
                "ending",
                json!({"duration_us":5000000, "background_asset_id":null}),
            ),
        ] {
            let mut upgraded = serde_json::to_value(&request).unwrap();
            let recipe = if upgraded["type"] == "build_video" {
                &mut upgraded["request"]["args"]["recipe"]
            } else {
                &mut upgraded["request"]["preset"]["content"]["recipe"]
            };
            recipe["presentation"][field.0] = field.1;
            let upgraded: SessionRequest = serde_json::from_value(upgraded).unwrap();
            assert_eq!(upgraded.minimum_protocol_minor(), 153);
        }
        let mut upgraded = serde_json::to_value(&request).unwrap();
        let recipe = if upgraded["type"] == "build_video" {
            &mut upgraded["request"]["args"]["recipe"]
        } else {
            &mut upgraded["request"]["preset"]["content"]["recipe"]
        };
        recipe["presentation"]["opening"] = json!({
            "duration_us":6000000,"background_asset_id":"image","logo":null,
            "audio":[],"performers":[],
            "theme":{"text":"テーマ","range":{"start_us":3000000,"duration_us":3000000},
                "transform":{"x":0,"y":0,"width":1920,"height":1080,"opacity":1},
                "font_size":64,"color":"#ffffff","outline":null,
                "layout":{"alignment":"center","wrap":true}}
        });
        let upgraded: SessionRequest = serde_json::from_value(upgraded).unwrap();
        assert_eq!(upgraded.minimum_protocol_minor(), 154);
    }
}

#[test]
fn role_keyed_psd_states_require_minor_155_for_build_and_preset() {
    let mut recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe["scenes"][0]["lines"][0]["psd_states"] = json!({"right":{
        "pose":{"source_sha256":"a".repeat(64),"selections":[]},
        "speech":{"character_id":"character","group":"/0","closed_option":"/0/0","open_option":"/0/1"}
    }});
    for request in [
        json!({"type":"build_video","request":{"type":"start","args":{
            "job_id":"job","project_path":"/tmp/project.vvm.json","recipe":recipe,"wait_ms":0
        }}}),
        json!({"type":"presets","request":{"action":"save","id":"template","expected_revision":0,
            "preset":{"schema_version":1,"name":"test","description":"","tags":[],
                "content":{"kind":"template","recipe":recipe}}}}),
    ] {
        let request: SessionRequest = serde_json::from_value(request).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 155);
    }
}

#[test]
fn section_order_requires_minor_156_for_build_and_preset() {
    let mut recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    recipe["presentation"] = json!({"standing_offset_y":0,"credits":null,"bgm_speech_envelope":null,
        "structure":{"blocks":["body","credits"],"title_duration_us":null}});
    for request in [
        json!({"type":"build_video","request":{"type":"start","args":{"job_id":"job","project_path":"/tmp/project.vvm.json","recipe":recipe,"wait_ms":0}}}),
        json!({"type":"presets","request":{"action":"save","id":"template","expected_revision":0,"preset":{"schema_version":1,"name":"test","description":"","tags":[],"content":{"kind":"template","recipe":recipe}}}}),
    ] {
        let request: SessionRequest = serde_json::from_value(request).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 156);
    }
}
