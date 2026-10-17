use sungeki_project::Project;
#[test]
fn vrm_render_plan_accepts_typed_model_and_motion_sources() {
    let p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/vrm-clip-project.json"
    ))
    .unwrap();
    assert!(sungeki_render::RenderPlan::new(&p, |_| Ok(())).is_ok());
}
