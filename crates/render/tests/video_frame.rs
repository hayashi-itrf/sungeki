#![cfg(unix)]
use std::{fs, path::PathBuf, sync::atomic::AtomicBool};
use sungeki_media::video_probe::frames::SourceFrame;
use sungeki_project::{AssetProbe, Project};
use sungeki_render::stage_video_frame;
fn frame() -> SourceFrame {
    let project: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json")).unwrap();
    let AssetProbe::Video(probe) = &project.assets[0].probe else {
        panic!()
    };
    let mut stream = probe.metadata.video.clone();
    stream.width = 2;
    stream.height = 1;
    stream.sample_aspect_ratio = None;
    stream.rotation_degrees = Some(90.0);
    stream.display_matrix = None;
    SourceFrame {
        rgba: vec![255, 0, 0, 255, 0, 0, 255, 255],
        stream,
        pts: 123,
        frame_index: 0,
    }
}
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn root() -> Root {
    let p = std::env::temp_dir().join(format!("video-frame-stage-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&p).unwrap();
    Root(p)
}
#[test]
fn normalized_resource_hash_no_overwrite_and_precancel() {
    use sha2::{Digest, Sha256};
    let root = root();
    let path = root.0.join("frame.rgba");
    let flag = AtomicBool::new(false);
    let resource = stage_video_frame(frame(), &path, &flag).unwrap();
    assert_eq!(
        (resource.decoded_video.width, resource.decoded_video.height),
        (1, 2)
    );
    assert_eq!(resource.decoded_video.pts, 123);
    let bytes = fs::read(&path).unwrap();
    assert_eq!(bytes, vec![0, 0, 255, 255, 255, 0, 0, 255]);
    assert_eq!(resource.sha256, format!("{:x}", Sha256::digest(&bytes)));
    assert!(stage_video_frame(frame(), &path, &flag).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let cancelled = root.0.join("cancelled");
    assert!(stage_video_frame(frame(), &cancelled, &AtomicBool::new(true)).is_err());
    assert!(!cancelled.exists());
}
#[test]
#[ignore = "requires configured image Python runtime"]
fn rust_resource_is_consumed_by_actual_image_compositor() {
    let root = root();
    let flag = AtomicBool::new(false);
    let resource = stage_video_frame(frame(), &root.0.join("frame.rgba"), &flag).unwrap();
    let json = root.0.join("resource.json");
    fs::write(&json, serde_json::to_vec(&resource).unwrap()).unwrap();
    let python = std::env::var_os("VVM_TEST_IMAGE_PYTHON").unwrap();
    let worker = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workers/image");
    let result = std::process::Command::new(python)
        .current_dir(worker)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .args([
            "-c",
            r#"
import json,sys
from PIL import Image
from render_frame import image_layer
resource=json.load(open(sys.argv[1]))
canvas=Image.new('RGBA',(4,4),'white')
image_layer(canvas,dict(asset_id='video'),dict(x=1,y=1,width=1,height=2),dict(video=resource))
assert canvas.getpixel((1,1))==(0,0,255,255)
assert canvas.getpixel((1,2))==(255,0,0,255)
assert canvas.getpixel((0,0))==(255,255,255,255)
"#,
        ])
        .arg(json)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn silent_and_selected_video_audio_plans_are_supported() {
    let mut project: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap();
    for visible in [true, false] {
        project.tracks[0].visible = visible;
        assert!(sungeki_render::RenderPlan::new(&project, |_| Ok(())).is_ok());
        assert!(sungeki_render::FramePlan::new(&project, 0).is_ok());
    }
    let AssetProbe::Video(probe) = &mut project.assets[0].probe else {
        panic!()
    };
    probe
        .metadata
        .audio
        .push(sungeki_project::VideoAudioStreamInfo {
            index: 1,
            codec: "aac".into(),
            sample_rate: 48000,
            channels: 2,
            clock: probe.metadata.video.clock.clone(),
        });
    let sungeki_project::ClipContent::Video { playback, .. } = &mut project.clips[0].content else {
        panic!()
    };
    playback.audio_stream = Some(1);
    for visible in [true, false] {
        project.tracks[0].visible = visible;
        assert!(sungeki_render::RenderPlan::new(&project, |_| Ok(())).is_ok());
        assert!(sungeki_render::FramePlan::new(&project, 0).is_ok());
    }
    project.tracks[0].muted = true;
    assert!(sungeki_render::RenderPlan::new(&project, |_| Ok(())).is_ok());
}
