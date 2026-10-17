use sungeki_project::{AssetProbe, ClipContent, Project, VideoAudioStreamInfo};
use sungeki_scene::{AudioSource, ScenePlan};
fn fixture() -> Project {
    let mut p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap();
    let AssetProbe::Video(probe) = &mut p.assets[0].probe else {
        panic!()
    };
    for index in [1, 2] {
        probe.metadata.audio.push(VideoAudioStreamInfo {
            index,
            codec: "aac".into(),
            sample_rate: 48000,
            channels: 2,
            clock: probe.metadata.video.clock.clone(),
        });
    }
    let ClipContent::Video { playback, .. } = &mut p.clips[0].content else {
        panic!()
    };
    playback.audio_stream = Some(1);
    playback.looped = true;
    playback.gain_db = -9.0;
    p
}
#[test]
fn video_visibility_and_embedded_audio_mute_are_independent() {
    let mut p = fixture();
    for visible in [true, false] {
        for muted in [true, false] {
            p.tracks[0].visible = visible;
            p.tracks[0].muted = muted;
            let plan = ScenePlan::new(&p).unwrap();
            assert_eq!(!plan.layers.is_empty(), visible);
            assert_eq!(plan.audio.len(), usize::from(!muted));
            if !muted {
                let cue = &plan.audio[0];
                let clip = &p.clips[0];
                assert_eq!(cue.clip_id, clip.id);
                assert_eq!(cue.start_us, clip.start_us);
                assert_eq!(cue.end_us.0, clip.start_us.0 + clip.duration_us.0);
                assert_eq!(cue.gain_db, Some(-9.0));
                let AudioSource::Video { playback, .. } = &cue.source else {
                    panic!()
                };
                let ClipContent::Video {
                    playback: expected, ..
                } = &clip.content
                else {
                    panic!()
                };
                assert_eq!(playback, expected);
            }
        }
    }
    p.tracks[0].muted = false;
    let ClipContent::Video { playback, .. } = &mut p.clips[0].content else {
        panic!()
    };
    playback.audio_stream = None;
    assert!(ScenePlan::new(&p).unwrap().audio.is_empty());
}
#[test]
fn decoded_audio_identity_distinguishes_streams_of_the_same_original() {
    let p = fixture();
    let plan = ScenePlan::new(&p).unwrap();
    let first = plan.audio[0].source.clone();
    let mut second = first.clone();
    let AudioSource::Video { playback, .. } = &mut second else {
        panic!()
    };
    playback.audio_stream = Some(2);
    assert_eq!(first.asset_id(), second.asset_id());
    assert_ne!(first.resource_id(), second.resource_id());
    assert!(first.resource_id().unwrap().ends_with(":1"));
    let AudioSource::Video { playback, .. } = &mut second else {
        panic!()
    };
    playback.audio_stream = None;
    assert!(second.resource_id().is_none());
}
