use sungeki_project::*;
#[test]
fn video_diagrams_require_explicit_valid_playback_and_image_diagrams_stay_images() {
    let mut r: BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    let p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap();
    let ClipContent::Video {
        asset_id,
        mut playback,
    } = p.clips[0].content.clone()
    else {
        panic!()
    };
    playback.looped = true;
    r.assets.push(p.assets[0].clone());
    let d = &mut r.scenes[0].diagrams[0];
    d.asset_id = asset_id;
    assert!(r.validate().is_err());
    r.scenes[0].diagrams[0].video_playback = Some(playback.clone());
    r.validate().unwrap();
    let encoded = serde_json::to_vec(&r).unwrap();
    assert_eq!(serde_json::from_slice::<BuildRecipe>(&encoded).unwrap(), r);
    r.scenes[0].diagrams[0]
        .video_playback
        .as_mut()
        .unwrap()
        .audio_stream = Some(u32::MAX);
    assert!(r.validate().is_err());
    r.scenes[0].diagrams[0].video_playback = Some(playback);
    r.scenes[0].diagrams[0].asset_id = r.scenes[0].background_asset_id.clone();
    assert!(r.validate().is_err());
}
