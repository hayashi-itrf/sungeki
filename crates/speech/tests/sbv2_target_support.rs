use serde_json::json;
use sungeki_project::*;
use sungeki_speech::sbv2::{Catalog, ModelConfig};
pub fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
pub fn environment() -> (Catalog, ModelConfig, Sbv2Identity) {
    let catalog = Catalog::parse(json!({"0":{"model_path":"/ssd/model",
        "config_path":"/ssd/config","device":"cpu","spk2id":{"声":0},
        "id2spk":{"0":"声"},"style2id":{"Neutral":0}}}))
    .unwrap();
    let config = ModelConfig::parse(
        &serde_json::to_vec(&json!({"version":"2.7.0-JP-Extra",
        "data":{"sampling_rate":44100,"spk2id":{"声":0},"num_styles":1,
        "style2id":{"Neutral":0}}}))
        .unwrap(),
    )
    .unwrap();
    let identity = Sbv2Identity {
        engine_revision: "fixture".into(),
        model_sha256: "a".repeat(64),
        config_sha256: config.sha256().into(),
        style_vectors_sha256: "b".repeat(64),
        text_encoder_fingerprint: "c".repeat(64),
    };
    (catalog, config, identity)
}
pub fn project(identity: &Sbv2Identity) -> Project {
    let mut project = Project::empty(id(1), "入力固定".into());
    project.characters.push(Character {
        id: id(2),
        name: "話者".into(),
        credit: "検証".into(),
        subtitle_style: SubtitleStyle {
            layout: None,
            outline: None,
            font_size: 64.0,
            color: "#000000".into(),
        },
        voice: Voice::Sbv2 {
            settings: serde_json::from_value(json!({"model_id":0,"speaker_id":0,
                "language":"JP","style":"Neutral","style_weight":1.,"length":1.,
                "sdp_ratio":0.2,"noise":0.6,"noisew":0.8,"auto_split":true,
                "split_interval":0.5}))
            .unwrap(),
            identity: identity.clone(),
        },
    });
    project.tracks.push(Track {
        id: id(3),
        name: "発話".into(),
        visible: true,
        muted: false,
        kind: TrackKind::Audio,
    });
    project.clips.push(Clip {
        id: id(4),
        track_id: id(3),
        scene_id: None,
        start_us: TimeUs(0),
        duration_us: TimeUs(1_000_000),
        transform: Transform {
            motion: None,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            opacity: 1.0,
            fade: None,
            flip_x: None,
        },
        content: ClipContent::Speech {
            gain_db: None,
            audio: None,
            character_id: id(2),
            subtitle_text: "字幕".into(),
            spoken_text: Some("よみ".into()),
        },
    });
    project
}
