use sungeki_protocol::*;
#[test]
fn video_recipe_requires_host_86() {
    let mut recipe: sungeki_project::BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    let p: sungeki_project::Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json")).unwrap();
    recipe.assets.push(p.assets[0].clone());
    let request = SessionRequest::BuildVideo {
        request: BuildVideoRequest::Start {
            args: StartBuildVideoArgs {
                wait_ms: None,
                reuse_from: None,
                managed_profiles: Default::default(),
                job_id: "job".into(),
                recipe,
                project_path: "/tmp/new.vvm.json".into(),
                base_url: "http://127.0.0.1:50021".into(),
                engine_version: "test".into(),
            },
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 86);
}
