use super::*;
fn frame() -> SourceFrame {
    let p: sungeki_project::Project = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/video-project.json"
    ))
    .unwrap();
    let sungeki_project::AssetProbe::Video(p) = &p.assets[0].probe else {
        panic!()
    };
    let mut stream = p.metadata.video.clone();
    stream.width = 3;
    stream.height = 2;
    stream.sample_aspect_ratio = None;
    stream.rotation_degrees = None;
    stream.display_matrix = None;
    SourceFrame {
        rgba: (1..=6).flat_map(|v| [v, 0, 0, 255]).collect(),
        stream,
        frame_index: 0,
        pts: 123,
    }
}
fn set_matrix(f: &mut SourceFrame, m: [i32; 4]) {
    f.stream.display_matrix = Some(format!(
        "00000000: {} {} 0\n00000001: {} {} 0\n00000002: 0 0 1073741824\n",
        m[0] * 65536,
        m[1] * 65536,
        m[2] * 65536,
        m[3] * 65536
    ));
}
#[test]
fn all_eight_orientations_keep_exact_pixels() {
    for (m, w, h, expected) in [
        ([1, 0, 0, 1], 3, 2, vec![1, 2, 3, 4, 5, 6]),
        ([0, -1, 1, 0], 2, 3, vec![3, 6, 2, 5, 1, 4]),
        ([-1, 0, 0, -1], 3, 2, vec![6, 5, 4, 3, 2, 1]),
        ([0, 1, -1, 0], 2, 3, vec![4, 1, 5, 2, 6, 3]),
        ([-1, 0, 0, 1], 3, 2, vec![3, 2, 1, 6, 5, 4]),
        ([1, 0, 0, -1], 3, 2, vec![4, 5, 6, 1, 2, 3]),
        ([0, 1, 1, 0], 2, 3, vec![1, 4, 2, 5, 3, 6]),
        ([0, -1, -1, 0], 2, 3, vec![6, 3, 5, 2, 4, 1]),
    ] {
        let mut f = frame();
        set_matrix(&mut f, m);
        let d = f.into_display().unwrap();
        assert_eq!((d.width, d.height), (w, h));
        assert_eq!(d.pts, 123);
        assert_eq!(
            d.rgba.chunks_exact(4).map(|p| p[0]).collect::<Vec<_>>(),
            expected
        );
    }
}
#[test]
fn aspect_is_applied_before_rotation_and_alpha_is_premultiplied() {
    let mut f = frame();
    f.stream.sample_aspect_ratio = Some(sungeki_project::MediaRatio {
        numerator: 2,
        denominator: 1,
    });
    f.stream.rotation_degrees = Some(90.0);
    let d = f.into_display().unwrap();
    assert_eq!((d.width, d.height), (2, 6));
    assert_eq!(
        sample(&[255, 0, 0, 0, 0, 0, 255, 255], 2, 1, 1.0, 0.5),
        [0, 0, 255, 128]
    );
    assert_eq!(sample(&[255, 0, 0, 0], 1, 1, 0.5, 0.5), [0, 0, 0, 0]);
}
#[test]
fn unsafe_or_unsupported_geometry_fails_explicitly() {
    for matrix in [
        "bad",
        "0: 65536 0 0\n1: 0 65536 0\n2: 1 0 1073741824",
        "0: 65536 0 1\n1: 0 65536 0\n2: 0 0 1073741824",
    ] {
        let mut f = frame();
        f.stream.display_matrix = Some(matrix.into());
        assert!(f.into_display().is_err());
    }
    let mut f = frame();
    set_matrix(&mut f, [0, 0, 0, 0]);
    assert!(f.into_display().is_err());
    let mut f = frame();
    f.stream.sample_aspect_ratio = Some(sungeki_project::MediaRatio {
        numerator: u32::MAX,
        denominator: 1,
    });
    assert!(f.into_display().is_err());
    let mut f = frame();
    f.rgba.pop();
    assert!(f.into_display().is_err());
}
