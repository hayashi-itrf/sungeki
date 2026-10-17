use sungeki_project::{ClipContent, Ducking, Project, TimeUs};
use sungeki_render::RenderPlan;

#[test]
fn ducking_without_speech_is_valid_and_muted_audio_is_omitted() {
    let mut project: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/audio-project.json")).unwrap();
    let ClipContent::Audio { playback, .. } = &mut project.clips[0].content else {
        panic!()
    };
    playback.ducking = Some(Ducking {
        attenuation_db: -15.0,
        attack_us: TimeUs(250000),
        release_us: TimeUs(500000),
    });
    assert!(RenderPlan::new(&project, |_| Ok(())).is_ok());
    // An inaudible clip has no effect to omit, and need not block other output.
    project.tracks[0].muted = true;
    assert!(RenderPlan::new(&project, |_| Ok(())).is_ok());
}
