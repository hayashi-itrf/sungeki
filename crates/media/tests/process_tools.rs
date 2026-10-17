#![cfg(unix)]
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_media::{ProcessSpec, run_process};

#[test]
#[ignore = "requires configured SSD runtime, image Python, font and ffmpeg/ffprobe"]
fn real_frame_worker_and_ffmpeg_use_the_owned_runner() {
    let runtime =
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").expect("VVM_MEDIA_TEST_ROOT"));
    assert!(runtime.is_absolute());
    let output = runtime.join(format!("owned-process-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&output).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let input = output.join("frame.json");
    fs::write(&input, r##"{"canvas":{"width":320,"height":180,"background":"#ffffff"},"frame_index":0,"time_us":0,"layers":[{"clip_id":"example","transform":{"x":16,"y":32,"width":288,"height":110,"opacity":1},"primitive":{"type":"text","text":"日本語字幕\n共通メディア実行","font_size":26,"color":"#205b30"}}]}"##).unwrap();
    let python = PathBuf::from(std::env::var_os("VVM_MEDIA_PYTHON").expect("VVM_MEDIA_PYTHON"));
    let font = PathBuf::from(std::env::var_os("VVM_MEDIA_FONT").expect("VVM_MEDIA_FONT"));
    use sha2::{Digest, Sha256};
    let font_hash = format!("{:x}", Sha256::digest(fs::read(&font).unwrap()));
    let png = output.join("frame.png");
    let args: Vec<OsString> = vec![
        root.join("workers/image/render_frame.py").into_os_string(),
        "--input".into(),
        input.into_os_string(),
        "--font".into(),
        font.into_os_string(),
        "--font-sha256".into(),
        font_hash.into(),
        "--output".into(),
        png.clone().into_os_string(),
    ];
    let cancelled = AtomicBool::new(false);
    let result = run_process(
        &ProcessSpec {
            program: &python,
            args: &args,
            directory: &output,
            timeout: Duration::from_secs(15),
            max_output_bytes: 1024 * 1024,
        },
        &cancelled,
    )
    .unwrap();
    assert!(result.succeeded(), "{result:?}");
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("\"issues\": []")
    );
    let probe = PathBuf::from(std::env::var_os("VVM_MEDIA_FFPROBE").expect("VVM_MEDIA_FFPROBE"));
    let args = [
        "-v".into(),
        "error".into(),
        "-show_entries".into(),
        "stream=width,height,codec_name".into(),
        "-of".into(),
        "default=noprint_wrappers=1".into(),
        png.clone().into_os_string(),
    ];
    let result = run_process(
        &ProcessSpec {
            program: &probe,
            args: &args,
            directory: &output,
            timeout: Duration::from_secs(10),
            max_output_bytes: 4096,
        },
        &cancelled,
    )
    .unwrap();
    assert!(result.succeeded(), "{result:?}");
    let metadata = String::from_utf8(result.stdout).unwrap();
    assert!(metadata.lines().any(|s| s == "codec_name=png"));
    assert!(metadata.lines().any(|s| s == "width=320"));
    assert!(metadata.lines().any(|s| s == "height=180"));
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_MEDIA_FFMPEG").expect("VVM_MEDIA_FFMPEG"));
    let args = [
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-i".into(),
        png.into_os_string(),
        "-f".into(),
        "null".into(),
        "-".into(),
    ];
    let result = run_process(
        &ProcessSpec {
            program: &ffmpeg,
            args: &args,
            directory: &output,
            timeout: Duration::from_secs(10),
            max_output_bytes: 4096,
        },
        &cancelled,
    )
    .unwrap();
    assert!(result.succeeded(), "{result:?}");
    println!(
        "owned frame worker / ffprobe / ffmpeg passed: {}",
        output.display()
    );
}
