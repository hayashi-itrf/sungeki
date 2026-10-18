use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_project::{AssetLocator, AssetProbe, Project, VideoProbe};
use sungeki_render::{FramePlan, RenderRuntime, render_frame_staged};
pub fn check(
    project: &Project,
    probe: &VideoProbe,
    source: &Path,
    root: &Path,
    ffmpeg: &Path,
    ffprobe: &Path,
) {
    let mut p = project.clone();
    p.assets[0].probe = AssetProbe::Video(probe.clone());
    p.assets[0].content_hash = Some(probe.sha256.clone());
    p.assets[0].locator = AssetLocator::Absolute {
        path: source.to_str().unwrap().into(),
    };
    p.canvas.width = 64;
    p.canvas.height = 48;
    p.canvas.fps.numerator = 5;
    p.canvas.fps.denominator = 1;
    p.clips[0].transform.x = 4.0;
    p.clips[0].transform.y = 6.0;
    p.clips[0].transform.width = 32.0;
    p.clips[0].transform.height = 24.0;
    let font = PathBuf::from("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc");
    let runtime = RenderRuntime {
        vrm: None,

        ffmpeg: ffmpeg.to_owned(),
        ffprobe: ffprobe.to_owned(),
        font: font.clone(),
        font_sha256: format!("{:x}", Sha256::digest(fs::read(&font).unwrap())),
        deadline: Duration::from_secs(30),
    };
    let mut hashes = Vec::new();
    for index in [0, 1] {
        let dir = root.join(format!("formal-{index}"));
        fs::create_dir(&dir).unwrap();
        let artifact = render_frame_staged(
            &FramePlan::new(&p, index).unwrap(),
            &runtime,
            &dir,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!((artifact.width, artifact.height), (64, 48));
        hashes.push(artifact.sha256);
        let resources: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("video-resources-0.json")).unwrap()).unwrap();
        let resource = resources.as_object().unwrap().values().next().unwrap();
        let raw = resource["path"].as_str().unwrap();
        let pixels = image::open(&artifact.path).unwrap().to_rgba8();
        assert_eq!(pixels.dimensions(), (64, 48));
        assert_eq!(
            image::imageops::crop_imm(&pixels, 4, 6, 32, 24)
                .to_image()
                .into_raw(),
            fs::read(raw).unwrap()
        );
        if let Some(evidence) = std::env::var_os("VVM_TEST_VIDEO_PNG_EVIDENCE") {
            if index == 0 {
                fs::copy(&artifact.path, PathBuf::from(evidence)).unwrap();
            }
        }
    }
    assert_ne!(hashes[0], hashes[1]);
}
