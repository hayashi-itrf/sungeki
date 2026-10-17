use serde_json::{Value, json};
use sungeki_speech::sbv2::{Catalog, Error, Language, Settings};
fn catalog() -> Value {
    json!({"0":{"model_path":"/ssd/model.safetensors","config_path":"/ssd/config.json",
        "device":"cpu","spk2id":{"声":0},"id2spk":{"0":"声"},"style2id":{"Neutral":0,"Happy":1}}})
}
fn settings() -> Settings {
    Settings {
        model_id: 0,
        speaker_id: 0,
        language: Language::JP,
        style: "Neutral".into(),
        style_weight: 1.,
        length: 1.,
        sdp_ratio: 0.2,
        noise: 0.6,
        noisew: 0.8,
        auto_split: true,
        split_interval: 0.5,
    }
}
#[test]
fn selection_requires_existing_model_speaker_style_and_changes_with_metadata() {
    let original = Catalog::parse(catalog()).unwrap();
    let selected = original.select(&settings()).unwrap();
    assert_eq!(selected.speaker_name, "声");
    assert_eq!(selected.metadata_sha256.len(), 64);
    for kind in 0..3 {
        let mut s = settings();
        match kind {
            0 => s.model_id = 1,
            1 => s.speaker_id = 1,
            _ => s.style = "missing".into(),
        }
        assert_eq!(original.select(&s).unwrap_err(), Error::Configuration);
    }
    let mut changed = catalog();
    changed["0"]["model_path"] = json!("/ssd/other.safetensors");
    assert_ne!(
        Catalog::parse(changed)
            .unwrap()
            .select(&settings())
            .unwrap(),
        selected
    );
    let reordered: Value =
        serde_json::from_str(&serde_json::to_string(&catalog()).unwrap()).unwrap();
    assert_eq!(
        Catalog::parse(reordered)
            .unwrap()
            .select(&settings())
            .unwrap(),
        selected
    );
}
#[test]
fn malformed_and_ambiguous_maps_are_rejected_but_empty_catalog_is_valid() {
    for kind in 0..5 {
        let mut value = catalog();
        match kind {
            0 => value["0"]["id2spk"]["0"] = json!("違う声"),
            1 => value["0"]["style2id"]["Happy"] = json!(0),
            2 => value["0"]["device"] = json!("cpu\n"),
            3 => value["0"]["spk2id"] = json!({}),
            _ => {
                let model = value["0"].take();
                value = json!({"00":model});
            }
        }
        assert!(Catalog::parse(value).is_err());
    }
    assert!(
        Catalog::parse(json!({}))
            .unwrap()
            .select(&settings())
            .is_err()
    );
}
#[tokio::test]
async fn metadata_change_after_synthesis_discards_received_audio() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::atomic::AtomicBool,
        time::Duration,
    };
    use sungeki_speech::sbv2::Sbv2Client;
    let original = catalog();
    let selected = Catalog::parse(original.clone())
        .unwrap()
        .select(&settings())
        .unwrap();
    let mut changed = original.clone();
    changed["0"]["model_path"] = json!("/changed");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        for (expected, body) in [
            ("GET", original.to_string()),
            ("POST", "unverified-wav".into()),
            ("GET", changed.to_string()),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut buf = [0; 2048];
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&buf[..n]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            assert!(String::from_utf8(request).unwrap().starts_with(expected));
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        }
    });
    let error = Sbv2Client::new(&url, Duration::from_secs(2))
        .unwrap()
        .synthesize_selected("本文", &settings(), &selected, &AtomicBool::new(false))
        .await
        .unwrap_err();
    assert_eq!(error, Error::ModelChanged);
    handle.join().unwrap();
}

#[test]
fn model_config_pins_format_and_checks_server_maps_and_language() {
    use sungeki_speech::sbv2::ModelConfig;
    let mut data = json!({"version":"2.7.0-JP-Extra","data":{"sampling_rate":44100,
        "spk2id":{"声":0},"num_styles":2,"style2id":{"Neutral":0,"Happy":1}}});
    let bytes = serde_json::to_vec(&data).unwrap();
    let config = ModelConfig::parse(&bytes).unwrap();
    let catalog = Catalog::parse(catalog()).unwrap();
    let expected = config.expected_audio(&catalog, &settings()).unwrap();
    assert_eq!((expected.sample_rate, expected.channels), (44100, 1));
    assert_eq!(config.sha256().len(), 64);
    let prepared = sungeki_speech::sbv2::PreparedSbv2Input::from_model_config(
        "本文",
        None,
        &[],
        &settings(),
        &catalog,
        &config,
    )
    .unwrap();
    assert_eq!(prepared.model_config_sha256(), Some(config.sha256()));
    let identity = sungeki_project::Sbv2Identity {
        engine_revision: "fixture-revision".into(),
        model_sha256: "a".repeat(64),
        config_sha256: config.sha256().into(),
        style_vectors_sha256: "b".repeat(64),
        text_encoder_fingerprint: "c".repeat(64),
    };
    let bound = prepared.clone().bind_identity(&identity).unwrap();
    assert_eq!(bound.identity(), Some(&identity));
    assert_ne!(bound.fingerprint(), prepared.fingerprint());
    assert!(bound.clone().bind_identity(&identity).is_err());
    for field in 0..4 {
        let mut changed = identity.clone();
        match field {
            0 => changed.engine_revision = "other".into(),
            1 => changed.model_sha256 = "d".repeat(64),
            2 => changed.style_vectors_sha256 = "d".repeat(64),
            _ => changed.text_encoder_fingerprint = "d".repeat(64),
        }
        assert_ne!(
            prepared
                .clone()
                .bind_identity(&changed)
                .unwrap()
                .fingerprint(),
            bound.fingerprint()
        );
    }
    let mut mismatch = identity.clone();
    mismatch.config_sha256 = "e".repeat(64);
    assert!(prepared.clone().bind_identity(&mismatch).is_err());

    let mut extra = data.clone();
    extra["model_name"] = json!("changed");
    let other = ModelConfig::parse(&serde_json::to_vec(&extra).unwrap()).unwrap();
    let changed_input = sungeki_speech::sbv2::PreparedSbv2Input::from_model_config(
        "本文",
        None,
        &[],
        &settings(),
        &catalog,
        &other,
    )
    .unwrap();
    assert_ne!(prepared.fingerprint(), changed_input.fingerprint());
    let mut english = settings();
    english.language = Language::EN;
    assert_eq!(
        config.expected_audio(&catalog, &english).unwrap_err(),
        Error::Configuration
    );
    data["data"]["spk2id"] = json!({"別の声":0});
    let changed = ModelConfig::parse(&serde_json::to_vec(&data).unwrap()).unwrap();
    assert_ne!(changed.sha256(), config.sha256());
    assert_eq!(
        changed.expected_audio(&catalog, &settings()).unwrap_err(),
        Error::ModelChanged
    );
    data["data"]["num_styles"] = json!(3);
    assert!(ModelConfig::parse(&serde_json::to_vec(&data).unwrap()).is_err());
    data["data"]["num_styles"] = json!(2);
    data["data"]["style2id"]["Happy"] = json!(9);
    assert!(ModelConfig::parse(&serde_json::to_vec(&data).unwrap()).is_err());
}
