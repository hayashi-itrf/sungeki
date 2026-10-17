use sungeki_editor::*;
use sungeki_project::*;
fn fixture() -> Project {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap()
}
fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
#[test]
fn portrait_fit_preserves_timing_assets_audio_and_motion_path() {
    let mut p = fixture();
    let clip = p
        .clips
        .iter_mut()
        .find(|c| matches!(c.content, ClipContent::Psd { .. }))
        .unwrap();
    clip.transform.x = 40.;
    clip.transform.y = 20.;
    clip.transform.width = 80.;
    clip.transform.height = 120.;
    clip.transform.motion = Some(VisualMotion {
        end_x: 100.,
        end_y: 50.,
        end_scale: 1.5,
        easing: MotionEasing::Linear,
    });
    let before = p.clone();
    let plan = plan_resize_layout(&p, 160, 320).unwrap();
    let resized = apply_batch(&p, &plan).unwrap();
    assert_eq!(p, before);
    let clip = resized
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Psd { .. }))
        .unwrap();
    assert_eq!(
        (
            clip.transform.x,
            clip.transform.y,
            clip.transform.width,
            clip.transform.height
        ),
        (20., 125., 40., 60.)
    );
    let motion = clip.transform.motion.as_ref().unwrap();
    assert_eq!(
        (motion.end_x, motion.end_y, motion.end_scale),
        (50., 140., 1.5)
    );
    assert_eq!(resized.assets, p.assets);
    assert_eq!(resized.scenes, p.scenes);
    assert_eq!(resized.groups, p.groups);
    assert_eq!(resized.audio_mix, p.audio_mix);
    assert_eq!(resized.canvas.fps, p.canvas.fps);
    for (a, b) in resized.clips.iter().zip(&p.clips) {
        assert_eq!(a.content, b.content);
        assert_eq!(a.start_us, b.start_us);
        assert_eq!(a.duration_us, b.duration_us);
        assert_eq!(a.scene_id, b.scene_id);
        assert_eq!(a.track_id, b.track_id);
    }
    assert_eq!(
        resized.characters[0].subtitle_style.font_size,
        p.characters[0].subtitle_style.font_size / 2.
    );
    let mut session = EditSession::new(p.clone()).unwrap();
    let request = EditRequest {
        operation_id: id(5000),
        expected_revision: 0,
        action: EditAction::Apply {
            commands: vec![Command::ResizeLayout {
                width: 160,
                height: 320,
            }],
        },
    };
    let receipt = session.execute(request.clone()).unwrap();
    assert_eq!(session.history().project(), &resized);
    assert_eq!(session.execute(request).unwrap(), receipt);
    session
        .execute(EditRequest {
            operation_id: id(5001),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: id(5000),
            },
        })
        .unwrap();
    assert_eq!(session.history().project(), &p);
}
#[test]
fn text_outline_blur_and_invalid_scale_are_handled_atomically() {
    let mut p = fixture();
    let mut text = p.clips[0].clone();
    text.id = id(6000);
    text.transform.motion = None;
    text.content = ClipContent::Text {
        text: "文字".into(),
        font_size: 20.,
        color: "#ffffff".into(),
        layout: None,
        outline: Some(SubtitleOutline {
            width: 3,
            color: "#000000".into(),
        }),
    };
    let mut blur = text.clone();
    blur.id = id(6001);
    blur.content = ClipContent::Blur { sigma: 10. };
    p.clips.extend([text, blur]);
    let resized = apply_batch(&p, &plan_resize_layout(&p, 640, 360).unwrap()).unwrap();
    assert!(matches!(&resized.clips[resized.clips.len()-2].content,
        ClipContent::Text { font_size,outline:Some(o),.. } if *font_size==40. && o.width==6));
    assert!(
        matches!(resized.clips.last().unwrap().content,ClipContent::Blur { sigma } if sigma==20.)
    );
    let before = p.clone();
    assert!(plan_resize_layout(&p, 0, 320).is_err());
    assert!(plan_resize_layout(&p, 6400, 3600).is_err());
    assert_eq!(p, before);
    assert_eq!(
        apply_batch(&p, &plan_resize_layout(&p, 320, 180).unwrap()).unwrap(),
        p
    );
}
