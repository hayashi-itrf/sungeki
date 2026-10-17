#[path = "proxy_worker_tests.rs"]
mod worker_tests;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
use sungeki_assets::proxy_store::{ProxyManifest, publish, read_candidate};
use sungeki_media::{
    ProcessSpec,
    proxy_timeline::ProxyTimeline,
    run_process,
    video_probe::{
        ProbeRuntime,
        frames::{PreparedVideo, proxy::ProxySettings},
        probe,
    },
};
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "requires real ffmpeg/ffprobe and configured SSD root"]
fn generated_proxy_survives_owner_drop_and_revalidates_after_reload() {
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let ffprobe = PathBuf::from(std::env::var_os("VVM_TEST_FFPROBE").unwrap());
    let root = Root(
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
            .join(format!("proxy-persist-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&root.0).unwrap();
    let cache = root.0.join("cache");
    let staging = root.0.join("staging");
    fs::create_dir(&cache).unwrap();
    fs::create_dir(&staging).unwrap();
    let cancel = AtomicBool::new(false);
    let args: Vec<_> = [
        "-nostdin",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=320x180:rate=10:duration=0.5",
        "-c:v",
        "libx264",
        "-bf",
        "0",
        "-an",
        "source.mp4",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    let result = run_process(
        &ProcessSpec {
            program: &ffmpeg,
            args: &args,
            directory: &root.0,
            timeout: Duration::from_secs(20),
            max_output_bytes: 1024 * 1024,
        },
        &cancel,
    )
    .unwrap();
    assert!(result.succeeded(), "{result:?}");
    let path = root.0.join("source.mp4");
    let before = fs::read(&path).unwrap();
    let runtime = ProbeRuntime {
        ffprobe: &ffprobe,
        root: &staging,
        timeout: Duration::from_secs(30),
        max_source_bytes: 1024 * 1024,
    };
    let source = probe(&path, &runtime, &cancel).unwrap();
    let original = PreparedVideo::open(&path, &source, &runtime, &cancel).unwrap();
    let settings = ProxySettings::new(160);
    let generated = original
        .generate_proxy(settings, &ffmpeg, &runtime, &cancel)
        .unwrap();
    let expected = generated
        .video
        .decode_at(
            sungeki_project::TimeUs(200000),
            &ffmpeg,
            Duration::from_secs(5),
            &cancel,
        )
        .unwrap()
        .unwrap();
    let manifest = ProxyManifest {
        version: sungeki_assets::proxy::PROXY_FORMAT_VERSION,
        source,
        derived: generated.video.probe().clone(),
        conversion: settings.descriptor().unwrap(),
        encoder_sha256: format!("{:x}", Sha256::digest(fs::read(&ffmpeg).unwrap())),
        source_pts: original.frame_pts().to_vec(),
        proxy_pts: generated.video.frame_pts().to_vec(),
    };
    let key = manifest.key().unwrap();
    let published = publish(&cache, &manifest, generated.snapshot_path(), &cancel).unwrap();
    drop(generated);
    drop(original);
    assert_eq!(fs::read_dir(&staging).unwrap().count(), 0);
    let restored = read_candidate(&cache, &key, &cancel).unwrap();
    assert_eq!(restored, manifest);
    let video = PreparedVideo::open(
        &published.join("video.mov"),
        &restored.derived,
        &runtime,
        &cancel,
    )
    .unwrap();
    assert_eq!(video.frame_pts(), restored.proxy_pts);
    let map = ProxyTimeline::verify(
        &restored.source.metadata.video.clock,
        &restored.source_pts,
        &video.probe().metadata.video.clock,
        video.frame_pts(),
    )
    .unwrap();
    let actual = video
        .decode_at(
            sungeki_project::TimeUs(200000),
            &ffmpeg,
            Duration::from_secs(5),
            &cancel,
        )
        .unwrap()
        .unwrap();
    assert_eq!(actual.rgba, expected.rgba);
    assert_eq!(
        actual.pts,
        map.frame_at(sungeki_project::TimeUs(200000))
            .unwrap()
            .unwrap()
            .proxy_pts
    );
    drop(video);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(worker_tests::run(
            &root.0, &path, &manifest, &ffmpeg, &ffprobe, &staging, &cache,
        ));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::read_dir(&staging).unwrap().count(), 0);
}
