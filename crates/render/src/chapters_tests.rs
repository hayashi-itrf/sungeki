use super::*;
use sungeki_project::{Scene, TimeUs};

fn fixture() -> Project {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap()
}
fn scene(name: &str, start: u64) -> Scene {
    Scene {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.into(),
        start_us: TimeUs(start),
        duration_us: TimeUs(100000),
        fade: None,
    }
}

#[test]
fn scenes_win_and_gaps_belong_to_previous_chapter_without_editing_timeline() {
    let mut project = fixture();
    project.title = "作品名".into();
    project.scenes = vec![scene("終わり", 2500001), scene("導入", 1000999)];
    let before = project.clone();
    let chapters = plan(&project, 3333);
    assert_eq!(
        chapters,
        vec![
            Chapter {
                start_ms: 0,
                end_ms: 1034,
                title: "作品名".into()
            },
            Chapter {
                start_ms: 1034,
                end_ms: 2534,
                title: "導入".into()
            },
            Chapter {
                start_ms: 2534,
                end_ms: 3333,
                title: "終わり".into()
            },
        ]
    );
    assert_eq!(project, before);
}

#[test]
fn simultaneous_speech_deduplicates_and_invisible_inaudible_speech_is_omitted() {
    let mut project = fixture();
    project.scenes.clear();
    project
        .clips
        .retain(|c| matches!(c.content, ClipContent::Speech { .. }));
    project.clips.truncate(1);
    project.clips[0].start_us = TimeUs(0);
    let mut second = project.clips[0].clone();
    second.start_us = TimeUs(0);
    project.clips.push(second);
    assert_eq!(plan(&project, 1000).len(), 1);
    for track in &mut project.tracks {
        track.visible = false;
        track.muted = true;
    }
    assert!(plan(&project, 1000).is_empty());
    for track in &mut project.tracks {
        track.muted = false;
    }
    assert_eq!(plan(&project, 1000).len(), 1);
    project.clips.clear();
    assert!(plan(&project, 1000).is_empty());
}

#[test]
fn metadata_cannot_inject_fields_or_chapters_and_titles_are_bounded() {
    let mut project = fixture();
    project.scenes = vec![scene("日本語\\=#;\r\n[CHAPTER]\nSTART=0\0", 0)];
    let chapters = plan(&project, 1000);
    assert_eq!(chapters[0].title, "日本語\\=#; [CHAPTER] START=0");
    let encoded = metadata(&chapters);
    assert_eq!(
        encoded,
        ";FFMETADATA1\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=1000\n"
    );
    assert_eq!(encoded.lines().filter(|s| *s == "[CHAPTER]").count(), 1);
    assert_eq!(title(&"あ".repeat(100)).chars().count(), 60);
    assert_eq!(title("\r\n\0"), "章");
}

#[test]
fn last_chapter_uses_encoded_frame_end_at_fractional_fps() {
    let mut project = Project::empty(uuid::Uuid::new_v4().to_string(), "端数FPS".into());
    project.canvas.fps = sungeki_project::FrameRate {
        numerator: 30000,
        denominator: 1001,
    };
    let mut item = scene("終端", 0);
    item.duration_us = TimeUs(1000001);
    project.scenes.push(item);
    let render = crate::RenderPlan::new(&project, |_| Ok(())).unwrap();
    assert_eq!(render.frame_count(), 30);
    assert_eq!(render.chapters[0].end_ms, 1001);
}

#[test]
fn chapter_seek_cannot_land_before_first_visible_frame() {
    let mut project = fixture();
    project.scenes = vec![scene("開始", 0), scene("第二章", 5289334)];
    assert_eq!(plan(&project, 11200)[1].start_ms, 5301);
    project.scenes[1].start_us = TimeUs(9172001);
    assert_eq!(plan(&project, 11200)[1].start_ms, 9201);
    project.canvas.fps = sungeki_project::FrameRate {
        numerator: 30000,
        denominator: 1001,
    };
    project.scenes = vec![scene("開始", 0), scene("A", 100), scene("B", 999)];
    let chapters = plan(&project, 1000);
    assert_eq!(chapters.len(), 2);
    assert_eq!(chapters[1].title, "A");
    assert_eq!(chapters[1].start_ms, 34);
    // Starts after the last encoded frame cannot create an empty tail chapter.
    project.scenes.push(scene("範囲外", 999999));
    assert_eq!(plan(&project, 1000), chapters);
}

#[cfg(unix)]
#[test]
fn verification_rejects_missing_misnamed_or_shifted_chapters() {
    let expected = vec![Chapter {
        start_ms: 0,
        end_ms: 1000,
        title: "導入".into(),
    }];
    let probe = serde_json::json!({"chapters":[{
        "start_time":"0.000000", "end_time":"1.000000", "tags":{"title":"導入"}
    }]});
    verify(&expected, &probe).unwrap();
    assert!(verify(&expected, &serde_json::json!({"chapters":[]})).is_err());
    for (key, value) in [("start_time", "NaN"), ("end_time", "1.001000")] {
        let mut changed = probe.clone();
        changed["chapters"][0][key] = value.into();
        assert!(verify(&expected, &changed).is_err());
    }
    let mut changed = probe.clone();
    changed["chapters"][0]["tags"]["title"] = "別名".into();
    assert!(verify(&expected, &changed).is_err());
}
