use super::*;
use serde_json::{Value, json};
fn stream() -> Value {
    json!({"index":0,"codec_type":"video","codec_name":"h264","width":720,"height":480,"sample_aspect_ratio":"8:9","time_base":"1/90000","start_pts":-9000,"duration_ts":90000,"avg_frame_rate":"30000/1001","r_frame_rate":"30000/1001","nb_frames":"30"})
}
fn read(streams: Vec<Value>) -> Result<VideoMetadata, ProbeError> {
    parse(
        &serde_json::to_vec(
            &json!({"format":{"format_name":"mov,mp4,m4a,3gp,3g2,mj2"},"streams":streams}),
        )
        .unwrap(),
    )
}
#[test]
fn preserves_exact_clocks_aspect_rotation_and_audio_selection() {
    let mut video = stream();
    video["side_data_list"] = json!([{"side_data_type":"Display Matrix","rotation":90,"displaymatrix":"original matrix"}]);
    let mut cover = stream();
    cover["index"] = json!(2);
    cover["disposition"] = json!({"attached_pic":1});
    let meta = read(vec![cover, video, json!({"index":1,"codec_type":"audio","codec_name":"aac","sample_rate":"48000","channels":2,"time_base":"1/48000","start_pts":1024,"duration_ts":48000})]).unwrap();
    assert_eq!(meta.video.index, 0);
    assert_eq!(meta.video.clock.start_pts, Some(-9000));
    assert_eq!(meta.video.clock.duration_ts, Some(90000));
    assert_eq!(
        meta.video.sample_aspect_ratio,
        Some(Rational {
            numerator: 8,
            denominator: 9
        })
    );
    assert_eq!(
        meta.video.average_frame_rate,
        Some(Rational {
            numerator: 30000,
            denominator: 1001
        })
    );
    assert_eq!(meta.video.rotation_degrees, Some(90.0));
    assert_eq!(
        meta.video.display_matrix.as_deref(),
        Some("original matrix")
    );
    assert_eq!(meta.audio[0].clock.start_pts, Some(1024));
}
#[test]
fn unknown_duration_and_frame_rate_are_not_invented() {
    let mut video = stream();
    video.as_object_mut().unwrap().remove("duration_ts");
    video["avg_frame_rate"] = json!("0/0");
    video["nb_frames"] = json!("N/A");
    let meta = read(vec![video]).unwrap();
    assert_eq!(meta.video.clock.duration_ts, None);
    assert_eq!(meta.video.average_frame_rate, None);
    assert_eq!(meta.video.frame_count, None);
    assert!(meta.audio.is_empty());
}
#[test]
fn invalid_and_ambiguous_streams_are_rejected() {
    assert!(read(vec![]).is_err());
    assert!(read(vec![stream(), stream()]).is_err());
    let mut other = stream();
    other["index"] = json!(1);
    assert!(read(vec![stream(), other]).is_err());
    for (field, value) in [
        ("time_base", json!("1/0")),
        ("duration_ts", json!(-1)),
        ("width", json!(0)),
        ("height", json!(20000)),
        ("avg_frame_rate", json!("NaN")),
        ("nb_frames", json!("-1")),
    ] {
        let mut video = stream();
        video[field] = value;
        assert!(read(vec![video]).is_err(), "{field}");
    }
}
