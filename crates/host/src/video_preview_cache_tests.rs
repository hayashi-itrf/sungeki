use super::*;
use crate::image_probe::video_preview::guard::OriginalStamp;
use std::{fs, path::PathBuf, sync::atomic::AtomicBool};
use sungeki_media::{
    ProcessSpec, run_process,
    video_probe::{ProbeRuntime, probe},
};
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
#[ignore = "requires real ffmpeg/ffprobe and configured SSD root"]
fn prepared_snapshots_are_bounded_evicted_expired_and_guarded() {
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let ffprobe = PathBuf::from(std::env::var_os("VVM_TEST_FFPROBE").unwrap());
    let root = Root(
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
            .join(format!("prepared-cache-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&root.0).unwrap();
    let source = root.0.join("source.mov");
    let args = [
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=64x64:rate=10:duration=0.2",
        "-c:v",
        "qtrle",
        "-pix_fmt",
        "argb",
        "source.mov",
    ]
    .map(Into::into);
    let flag = AtomicBool::new(false);
    assert!(
        run_process(
            &ProcessSpec {
                program: &ffmpeg,
                args: &args,
                directory: &root.0,
                timeout: Duration::from_secs(10),
                max_output_bytes: 1024 * 1024
            },
            &flag
        )
        .unwrap()
        .succeeded()
    );
    let runtime = ProbeRuntime {
        ffprobe: &ffprobe,
        root: &root.0,
        timeout: Duration::from_secs(10),
        max_source_bytes: 1024 * 1024,
    };
    let probe = probe(&source, &runtime, &flag).unwrap();
    let prepare = || {
        let work = Work::named(&root.0, "prepared-video").unwrap();
        let runtime = ProbeRuntime {
            root: &work.path,
            ..runtime
        };
        let video = PreparedVideo::open(&source, &probe, &runtime, &flag).unwrap();
        let guards = [source.as_path(), video.snapshot_path()]
            .into_iter()
            .map(|p| (p.to_owned(), OriginalStamp::read(p).unwrap()))
            .collect();
        Arc::new(Prepared {
            video,
            guards,
            _work: work,
        })
    };
    let mut cache = Cache::default();
    let mut paths = Vec::new();
    for n in 0..4 {
        let entry = prepare();
        paths.push(entry._work.path.clone());
        cache.insert(n.to_string(), entry);
    }
    assert_eq!(cache.entries.len(), 3);
    assert!(!paths[0].exists());
    drop(cache.get("1").unwrap().unwrap());
    cache.insert("4".into(), prepare());
    assert!(!paths[2].exists());
    cache.entries.front_mut().unwrap().1 = Instant::now() - Duration::from_secs(31);
    assert!(cache.get("3").unwrap().is_none());
    assert!(!paths[3].exists());
    let entry = cache.get("1").unwrap().unwrap();
    let private_path = entry.video.snapshot_path().to_owned();
    fs::write(&private_path, b"modified private snapshot").unwrap();
    drop(entry);
    assert!(cache.get("1").is_err());
    assert!(!paths[1].exists());
    cache.clear();
    let first = prepare();
    let directory = first._work.path.clone();
    cache.insert_with_limits("a".into(), first, 3, probe.byte_length);
    cache.insert_with_limits("b".into(), prepare(), 3, probe.byte_length);
    assert_eq!(cache.entries.len(), 1);
    assert!(!directory.exists());
    let too_big = prepare();
    let directory = too_big._work.path.clone();
    cache.insert_with_limits("oversized".into(), too_big, 3, probe.byte_length - 1);
    assert!(!directory.exists());
    assert_eq!(cache.entries.len(), 1);
    cache.clear();
    assert!(
        fs::read_dir(&root.0)
            .unwrap()
            .all(|e| e.unwrap().path() == source)
    );
}
