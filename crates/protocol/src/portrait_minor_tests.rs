use crate::*;
#[test]
fn portrait_build_requires_report_support_and_foreword_remains_minor_65() {
    let mut recipe: sungeki_project::BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-narration.json"
    ))
    .unwrap();
    recipe.preset_id = "narration-portrait-v1".into();
    let request = SessionRequest::BuildVideo {
        request: BuildVideoRequest::Start {
            args: StartBuildVideoArgs {
                wait_ms: None,
                reuse_from: None,
                managed_profiles: Default::default(),
                job_id: "job".into(),
                recipe,
                project_path: "/tmp/new.json".into(),
                base_url: "http://127.0.0.1:50021".into(),
                engine_version: "test".into(),
            },
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 73);
    let request = SessionRequest::PreviewEdit {
        session_id: "session".into(),
        expected_revision: 0,
        offset: 0,
        limit: 20,
        commands: vec![sungeki_editor::Command::PrependForeword {
            template: sungeki_editor::ForewordTemplate {
                preset_id: "foreword-portrait-v1".into(),
                background_asset_id: "asset".into(),
                text: "前書き".into(),
                duration_us: sungeki_project::TimeUs(8_000_000),
                scene_id: "scene".into(),
                track_id: "track".into(),
                clip_ids: vec![],
            },
        }],
    };
    assert_eq!(request.minimum_protocol_minor(), 65);
}
