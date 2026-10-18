use super::*;
use ag_psd::psd::{BlendMode, ColorMode, Layer, LayerAdditionalInfo, PixelData, Psd, WriteOptions};
use sha2::{Digest, Sha256};
use sungeki_project::{PsdPose, PsdSelection};
fn pixel(name: &str, color: [u8; 4], hidden: bool) -> Layer {
    Layer {
        additional_info: LayerAdditionalInfo {
            name: Some(name.into()),
            ..Default::default()
        },
        left: Some(0.),
        top: Some(0.),
        right: Some(2.),
        bottom: Some(2.),
        hidden: Some(hidden),
        opacity: Some(1.),
        blend_mode: Some(BlendMode::Normal),
        image_data: Some(PixelData {
            width: 2,
            height: 2,
            data: color.repeat(4),
        }),
        ..Default::default()
    }
}
#[test]
fn original_layer_pose_selection_and_multiply_without_embedded_preview() {
    let mut multiply = pixel("multiply", [128, 255, 128, 255], false);
    multiply.blend_mode = Some(BlendMode::Multiply);
    let group = Layer {
        additional_info: LayerAdditionalInfo {
            name: Some("mouth".into()),
            ..Default::default()
        },
        blend_mode: Some(BlendMode::PassThrough),
        children: Some(vec![
            pixel("*closed", [255, 0, 0, 255], false),
            pixel("*open", [0, 255, 0, 255], true),
        ]),
        ..Default::default()
    };
    let fixture = Psd {
        width: 2.,
        height: 2.,
        bits_per_channel: Some(8.),
        color_mode: Some(ColorMode::Rgb),
        children: Some(vec![group, multiply]),
        ..Default::default()
    };
    let bytes = ag_psd::write_psd(&fixture, &WriteOptions::default());
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let doc = psd::Document::read(&bytes, &hash).unwrap();
    assert_eq!(doc.probe.layers[0].name, "mouth");
    let mut pose = PsdPose {
        source_sha256: hash.clone(),
        selections: vec![],
    };
    assert_eq!(
        doc.render(&pose).unwrap().get_pixel(0, 0).0,
        [128, 0, 0, 255]
    );
    pose.selections.push(PsdSelection {
        group: "/0".into(),
        option: "/0/1".into(),
    });
    assert_eq!(
        doc.render(&pose).unwrap().get_pixel(0, 0).0,
        [0, 255, 0, 255]
    );
    pose.selections[0].option = "/1".into();
    assert!(doc.render(&pose).is_err());
    assert!(psd::Document::read(&bytes, &"0".repeat(64)).is_err());
    assert!(
        psd::Document::read(&bytes[..20], &format!("{:x}", Sha256::digest(&bytes[..20]))).is_err()
    );
}
#[test]
fn native_shape_opacity_clipping_and_cancel() {
    use serde_json::json;
    use std::sync::atomic::AtomicBool;
    let packet = json!({"canvas":{"width":4,"height":4,"background":"#ffffff"},"frame_index":0,"time_us":0,
    "layers":[{"clip_id":"shape","transform":{"x":-1,"y":1,"width":3,"height":2,"opacity":0.5},"primitive":{"type":"shape","shape":"rectangle","fill":"#000000"}}]});
    let mut raster = frame::Rasterizer::default();
    let (image, report) = raster
        .render(&packet, &json!({}), b"", "hash", &AtomicBool::new(false))
        .unwrap();
    assert_eq!(image.get_pixel(0, 1).0, [128, 128, 128, 255]);
    assert_eq!(image.get_pixel(2, 1).0, [255, 255, 255, 255]);
    assert_eq!(report["issues"], json!([]));
    assert!(
        raster
            .render(&packet, &json!({}), b"", "hash", &AtomicBool::new(true))
            .is_err()
    );
}
#[test]
fn wrapping_preserves_explicit_breaks_and_japanese_punctuation() {
    let rows = wrapping::lines("これは、確認です。\n次の行", 6., true, |s| {
        s.chars().count() as f64
    });
    assert!(rows.iter().all(|s| !s.starts_with(['、', '。'])));
    assert_eq!(rows.concat(), "これは、確認です。次の行");
    assert_eq!(
        wrapping::lines("a\r\n\r\nb", 10., false, |s| s.len() as f64),
        vec!["a", "", "b"]
    );
}
#[test]
fn verified_original_rejects_links_and_changed_content() {
    use std::fs;
    let dir = std::env::temp_dir().join(format!("sungeki-raster-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("original");
    fs::write(&file, b"original").unwrap();
    let hash = format!("{:x}", Sha256::digest(b"original"));
    assert_eq!(frame::verified(&file, &hash).unwrap(), b"original");
    #[cfg(unix)]
    {
        let link = dir.join("link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(frame::verified(&link, &hash).is_err());
    }
    fs::write(&file, b"modified").unwrap();
    assert!(frame::verified(&file, &hash).is_err());
    assert!(frame::verified(&dir, &hash).is_err());
    fs::remove_dir_all(dir).unwrap();
}
