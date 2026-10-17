use super::*;
use crate::FrameSample;
fn failed(index: u64) -> ReviewedFrame {
    ReviewedFrame {
        sample: FrameSample {
            frame_index: index,
            time_us: TimeUs(index * 1000),
            reasons: vec![],
        },
        output: None,
        error: Some("failure with arbitrary long details".into()),
    }
}
#[test]
fn failed_cells_keep_their_positions_and_labels_and_pages_are_bounded() {
    let frames: Vec<_> = (0..12).map(failed).collect();
    let p = document(&frames, 1, 2).unwrap();
    sungeki_project::validate(&p).unwrap();
    assert_eq!((p.canvas.width, p.canvas.height), (1216, 1316));
    let labels: Vec<_> = p
        .clips
        .iter()
        .filter_map(|c| match &c.content {
            ClipContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(labels.contains(&"00:00:00.011"));
    assert!(labels.contains(&"frame 11  /  生成失敗"));
    assert_eq!(
        labels
            .iter()
            .filter(|s| **s == "画像を生成できませんでした")
            .count(),
        12
    );
    assert!(p.assets.is_empty());
    assert!(document(&[], 1, 1).is_err());
    assert!(document(&frames, 0, 1).is_err());
    assert!(document(&frames, 2, 1).is_err());
    assert!(document(&(0..13).map(failed).collect::<Vec<_>>(), 1, 1).is_err());
}
#[test]
fn verified_png_metadata_is_used_and_frame_mismatch_or_modification_is_rejected() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/transparent.png");
    let probe = sungeki_assets::probe_image(&path, sungeki_assets::ProbeLimits::default()).unwrap();
    let mut frame = failed(42);
    frame.error = None;
    frame.output = Some(FrameArtifact {
        path,
        sha256: probe.sha256.clone(),
        byte_length: probe.byte_length,
        width: probe.width,
        height: probe.height,
        frame_index: 42,
        time_us: TimeUs(42000),
        font_sha256: "0".repeat(64),
        layout_version: 8,
    });
    let p = document(std::slice::from_ref(&frame), 1, 1).unwrap();
    assert_eq!(p.assets[0].probe, AssetProbe::Image(probe));
    let clip = p
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Image { .. }))
        .unwrap();
    assert!(
        (clip.transform.width / clip.transform.height
            - frame.output.as_ref().unwrap().width as f64
                / frame.output.as_ref().unwrap().height as f64)
            .abs()
            < 0.001
    );
    assert!(clip.transform.width <= 384.0 && clip.transform.height <= 216.0);
    frame.output.as_mut().unwrap().frame_index = 43;
    assert!(document(std::slice::from_ref(&frame), 1, 1).is_err());
    frame.output.as_mut().unwrap().frame_index = 42;
    frame.output.as_mut().unwrap().sha256 = "0".repeat(64);
    assert!(document(&[frame], 1, 1).is_err());
}
#[test]
fn time_labels_include_hours_and_millisecond_precision() {
    assert_eq!(timestamp(TimeUs(3_661_234_567)), "01:01:01.234");
    assert_eq!(timestamp(TimeUs(604_799_999_999)), "167:59:59.999");
}
