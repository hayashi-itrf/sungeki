#[cfg(unix)]
fn main() {
    use std::{fs, path::Path, sync::atomic::AtomicBool};
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        5,
        "PROJECT_JSON RUNTIME_JSON EMPTY_DIRECTORY LIMIT"
    );
    let project = serde_json::from_slice(&fs::read(&args[1]).unwrap()).unwrap();
    let runtime = serde_json::from_slice(&fs::read(&args[2]).unwrap()).unwrap();
    let report = sungeki_review::review_representative_frames(
        &project,
        &[],
        args[4].parse().unwrap(),
        0,
        &runtime,
        Path::new(&args[3]),
        &AtomicBool::new(false),
    )
    .unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
#[cfg(not(unix))]
fn main() {
    panic!("formal render requires Unix");
}
