use serde_json::json;
use std::io::Cursor;
use sungeki_media::{ExpectedAudio, WavError};
use sungeki_project::PronunciationEntry;
use sungeki_speech::sbv2::{Catalog, Language, PreparedSbv2Input, Settings};
fn catalog() -> Catalog {
    Catalog::parse(
        json!({"0":{"model_path":"/ssd/model","config_path":"/ssd/config",
        "device":"cpu","spk2id":{"声":0},"id2spk":{"0":"声"},"style2id":{"Neutral":0}}}),
    )
    .unwrap()
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
fn expected() -> ExpectedAudio {
    ExpectedAudio {
        sample_rate: 44100,
        channels: 1,
    }
}
#[test]
fn snapshot_keeps_sources_and_reading_and_fingerprint_tracks_every_input() {
    let mut dictionary = vec![PronunciationEntry {
        written: "Codex".into(),
        spoken: "コーデックス".into(),
    }];
    let mut settings = settings();
    let first = PreparedSbv2Input::new(
        "Codex",
        None,
        &dictionary,
        &settings,
        &catalog(),
        expected(),
    )
    .unwrap();
    assert_eq!(first.resolved().text, "コーデックス");
    assert_eq!(first.subtitle(), "Codex");
    assert_eq!(first.spoken(), None);
    dictionary[0].spoken = "別の読み".into();
    settings.length = 1.5;
    assert_eq!(first.dictionary()[0].spoken, "コーデックス");
    assert_eq!(first.settings().length, 1.);
    let cases = [
        PreparedSbv2Input::new(
            "Codex",
            None,
            &dictionary,
            first.settings(),
            &catalog(),
            expected(),
        )
        .unwrap(),
        PreparedSbv2Input::new(
            "Codex",
            None,
            first.dictionary(),
            &settings,
            &catalog(),
            expected(),
        )
        .unwrap(),
        PreparedSbv2Input::new(
            "別字幕",
            Some("コーデックス"),
            first.dictionary(),
            first.settings(),
            &catalog(),
            expected(),
        )
        .unwrap(),
        PreparedSbv2Input::new(
            "Codex",
            None,
            first.dictionary(),
            first.settings(),
            &catalog(),
            ExpectedAudio {
                sample_rate: 48000,
                channels: 1,
            },
        )
        .unwrap(),
    ];
    for changed in cases {
        assert_ne!(first.fingerprint(), changed.fingerprint());
    }
    assert_eq!(
        first.fingerprint(),
        PreparedSbv2Input::new(
            "Codex",
            None,
            first.dictionary(),
            first.settings(),
            &catalog(),
            expected()
        )
        .unwrap()
        .fingerprint()
    );
}
fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(
        &mut cursor,
        hound::WavSpec {
            channels: 1,
            sample_rate: rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for sample in samples {
        writer.write_sample(*sample).unwrap();
    }
    writer.finalize().unwrap();
    cursor.into_inner()
}
#[test]
fn verifies_actual_frames_and_rejects_bad_rate_empty_truncated_nonfinite_audio() {
    let input =
        PreparedSbv2Input::new("本文", None, &[], &settings(), &catalog(), expected()).unwrap();
    let bytes = wav(&[0.25], 44100);
    let audio = input.verify_audio(bytes.clone()).unwrap();
    assert_eq!(audio.info().frames, 1);
    assert_eq!(audio.info().duration_us, 23);
    assert_eq!(audio.bytes(), bytes);
    assert_eq!(audio.info().content_hash.len(), 64);
    assert_eq!(
        input.verify_audio(wav(&[0.], 48000)).unwrap_err(),
        WavError::FormatMismatch
    );
    assert_eq!(
        input.verify_audio(wav(&[], 44100)).unwrap_err(),
        WavError::Empty
    );
    assert_eq!(
        input.verify_audio(wav(&[f32::NAN], 44100)).unwrap_err(),
        WavError::NonFiniteSample
    );
    let mut truncated = bytes;
    truncated.pop();
    assert!(input.verify_audio(truncated).is_err());
    assert!(input.verify_audio(b"server error".to_vec()).is_err());
}
#[test]
fn rejects_bad_text_format_and_selection_before_synthesis() {
    assert!(PreparedSbv2Input::new("", None, &[], &settings(), &catalog(), expected()).is_err());
    assert!(
        PreparedSbv2Input::new(
            "本文",
            None,
            &[],
            &settings(),
            &catalog(),
            ExpectedAudio {
                sample_rate: 0,
                channels: 1
            }
        )
        .is_err()
    );
    let mut s = settings();
    s.model_id = 55;
    assert!(PreparedSbv2Input::new("本文", None, &[], &s, &catalog(), expected()).is_err());
}

#[tokio::test]
async fn prepared_dispatch_requires_bound_identity_before_network() {
    use std::{sync::atomic::AtomicBool, time::Duration};
    use sungeki_speech::sbv2::{Error, Sbv2Client};
    let input =
        PreparedSbv2Input::new("本文", None, &[], &settings(), &catalog(), expected()).unwrap();
    let client = Sbv2Client::new("http://127.0.0.1:1", Duration::from_secs(1)).unwrap();
    assert_eq!(
        client
            .synthesize_prepared(&input, &AtomicBool::new(false))
            .await
            .unwrap_err(),
        Error::Configuration
    );
}
