use super::*;
#[test]
fn video_follows_speech_range_and_rejects_a_too_short_nonlooped_selection() {
    let (p, mut r, speech) = recipe_with_bgm();
    let video: Project = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap();
    let ClipContent::Video {
        asset_id,
        mut playback,
    } = video.clips[0].content.clone()
    else {
        panic!()
    };
    playback.looped = true;
    r.assets.push(video.assets[0].clone());
    r.scenes[0].diagrams[0].asset_id = asset_id.clone();
    r.scenes[0].diagrams[0].video_playback = Some(playback.clone());
    let stage = synthesize(&p, &r, &speech);
    let ids = layout_ids();
    let commands = plan_build_layout(&stage, &r, &speech, &ids).unwrap();
    let output = apply_batch(&stage, &commands).unwrap();
    let clip = output
        .clips
        .iter()
        .find(|c| c.id == ids.scenes[0].diagrams[0])
        .unwrap();
    assert_eq!(clip.content, ClipContent::Video { asset_id, playback });
    let first = output
        .clips
        .iter()
        .find(|c| c.id == speech.speech_ids[0])
        .unwrap();
    let last = output
        .clips
        .iter()
        .find(|c| c.id == speech.speech_ids[1])
        .unwrap();
    assert_eq!(clip.start_us, first.start_us);
    assert_eq!(
        clip.start_us.0 + clip.duration_us.0,
        last.start_us.0 + last.duration_us.0
    );
    assert_eq!(clip.scene_id, first.scene_id);
    let before = stage.clone();
    r.scenes[0].diagrams[0]
        .video_playback
        .as_mut()
        .unwrap()
        .looped = false;
    assert!(plan_build_layout(&stage, &r, &speech, &ids).is_err());
    assert_eq!(stage, before);
}
