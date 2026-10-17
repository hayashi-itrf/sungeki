use sungeki_project::Project;
use sungeki_scene::{AudioSource, ScenePlan};
#[test]
fn audio_selection_survives_scene_planning_without_visual_layer() {
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/audio-project.json")).unwrap();
    let plan = ScenePlan::new(&p).unwrap();
    assert!(plan.layers.is_empty());
    assert_eq!(plan.audio.len(), 1);
    assert!(matches!(plan.audio[0].source, AudioSource::Material { .. }));
    p.tracks[0].visible = false;
    assert_eq!(ScenePlan::new(&p).unwrap().audio, plan.audio);
    p.tracks[0].muted = true;
    let plan = ScenePlan::new(&p).unwrap();
    assert!(plan.layers.is_empty() && plan.audio.is_empty());
    assert_eq!(plan.duration_us.0, 2_000_000);
}
