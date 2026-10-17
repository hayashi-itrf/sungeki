mod http_support;
use http_support::{Reply, query, server, settings};
use std::{sync::atomic::AtomicBool, time::Duration};
use sungeki_project::{Project, PronunciationEntry};
use sungeki_speech::{
    PreparedVoicevoxInput, VoicevoxIdentity,
    voicevox::{TransportError, VoicevoxClient},
};

fn identity() -> VoicevoxIdentity {
    VoicevoxIdentity {
        engine_id: "test-engine".into(),
        engine_version: "1.0".into(),
        core_version: "0.16.0".into(),
    }
}
fn dictionary() -> Vec<PronunciationEntry> {
    vec![PronunciationEntry {
        written: "ABC".into(),
        spoken: "エービーシー".into(),
    }]
}
fn input(spoken: Option<&str>) -> PreparedVoicevoxInput {
    PreparedVoicevoxInput::new("ABCです", spoken, &dictionary(), &settings(), &identity()).unwrap()
}
#[test]
fn snapshot_owns_all_inputs_and_preserves_dictionary_application_evidence() {
    let mut dictionary = dictionary();
    let mut settings = settings();
    let mut identity = identity();
    let prepared =
        PreparedVoicevoxInput::new("ABCです", None, &dictionary, &settings, &identity).unwrap();
    let fingerprint = prepared.fingerprint().to_owned();
    dictionary[0].spoken = "変更後".into();
    settings.speed_scale = 2.0;
    identity.core_version = "later".into();
    assert_eq!(prepared.resolved().text, "エービーシーです");
    assert_eq!(prepared.resolved().applied[0].dictionary_index, 0);
    assert_eq!(prepared.dictionary()[0].spoken, "エービーシー");
    assert_eq!(prepared.settings().speed_scale, 1.2);
    assert_eq!(prepared.identity().core_version, "0.16.0");
    assert_eq!(prepared.subtitle(), "ABCです");
    assert_eq!(prepared.spoken(), None);
    assert_eq!(prepared.fingerprint(), fingerprint);
    assert_eq!(input(None).fingerprint(), fingerprint);
}
#[test]
fn explicit_reading_and_subtitle_changes_are_distinct_even_with_equal_audio_text() {
    let implicit = input(None);
    let explicit = input(Some("エービーシーです"));
    assert_eq!(implicit.resolved().text, explicit.resolved().text);
    assert_ne!(implicit.fingerprint(), explicit.fingerprint());
    assert!(explicit.resolved().applied.is_empty());
    let changed = PreparedVoicevoxInput::new(
        "字幕だけ変更",
        explicit.spoken(),
        explicit.dictionary(),
        explicit.settings(),
        explicit.identity(),
    )
    .unwrap();
    assert_eq!(changed.resolved().text, explicit.resolved().text);
    assert_ne!(changed.fingerprint(), explicit.fingerprint());
    let mut dictionary = dictionary();
    dictionary.push(PronunciationEntry {
        written: "未使用".into(),
        spoken: "みしよう".into(),
    });
    let changed = PreparedVoicevoxInput::new(
        explicit.subtitle(),
        explicit.spoken(),
        &dictionary,
        explicit.settings(),
        explicit.identity(),
    )
    .unwrap();
    assert_ne!(
        changed.dictionary_fingerprint(),
        explicit.dictionary_fingerprint()
    );
    assert_ne!(changed.fingerprint(), explicit.fingerprint());
}
#[test]
fn every_voice_control_and_engine_identity_changes_the_fingerprint() {
    let original = input(None);
    for field in 0..12 {
        let mut voice = settings();
        match field {
            0 => voice.style_id += 1,
            1 => voice.speed_scale += 0.1,
            2 => voice.pitch_scale += 0.1,
            3 => voice.intonation_scale += 0.1,
            4 => voice.volume_scale += 0.1,
            5 => voice.pre_phoneme_length += 0.1,
            6 => voice.post_phoneme_length += 0.1,
            7 => voice.pause_length = Some(0.2),
            8 => voice.pause_length_scale += 0.1,
            9 => voice.output_sampling_rate += 1,
            10 => voice.output_stereo = true,
            _ => voice.enable_interrogative_upspeak = true,
        }
        let changed = PreparedVoicevoxInput::new(
            original.subtitle(),
            None,
            original.dictionary(),
            &voice,
            original.identity(),
        )
        .unwrap();
        assert_ne!(
            changed.fingerprint(),
            original.fingerprint(),
            "control {field}"
        );
    }
    for field in 0..3 {
        let mut identity = identity();
        match field {
            0 => identity.engine_id.push('2'),
            1 => identity.engine_version.push('2'),
            _ => identity.core_version.push('2'),
        }
        let changed = PreparedVoicevoxInput::new(
            original.subtitle(),
            None,
            original.dictionary(),
            original.settings(),
            &identity,
        )
        .unwrap();
        assert_ne!(
            changed.fingerprint(),
            original.fingerprint(),
            "identity {field}"
        );
    }
}
#[test]
fn unrelated_project_changes_do_not_invalidate_audio_input() {
    let mut project = Project::empty(
        "00000000-0000-0000-0000-000000000001".into(),
        "before".into(),
    );
    project.pronunciation_dictionary = dictionary();
    let before = PreparedVoicevoxInput::new(
        "ABCです",
        None,
        &project.pronunciation_dictionary,
        &settings(),
        &identity(),
    )
    .unwrap();
    project.title = "after".into();
    project.canvas.background = "#112233".into();
    let after = PreparedVoicevoxInput::new(
        "ABCです",
        None,
        &project.pronunciation_dictionary,
        &settings(),
        &identity(),
    )
    .unwrap();
    assert_eq!(before.fingerprint(), after.fingerprint());
}
#[test]
fn rejects_invalid_inputs_instead_of_fingerprinting_silent_fallbacks() {
    for spoken in [Some(""), Some(" "), Some("a\0b")] {
        assert!(
            PreparedVoicevoxInput::new("ABC", spoken, &dictionary(), &settings(), &identity())
                .is_err()
        );
    }
    let mut voice = settings();
    voice.speed_scale = f64::NAN;
    assert!(PreparedVoicevoxInput::new("ABC", None, &dictionary(), &voice, &identity()).is_err());
    let mut engine = identity();
    engine.engine_version.clear();
    assert!(PreparedVoicevoxInput::new("ABC", None, &dictionary(), &settings(), &engine).is_err());
    assert!(
        PreparedVoicevoxInput::new(
            &"x".repeat(65537),
            Some("ア"),
            &dictionary(),
            &settings(),
            &identity()
        )
        .is_err()
    );
}
#[tokio::test]
async fn transport_uses_frozen_reading_and_rejects_mismatched_core_before_sending() {
    let input = input(None);
    let (base, mock) = server(vec![Reply::ok(query()), Reply::ok(b"unverified".to_vec())]).await;
    let client = VoicevoxClient::new(&base, "0.16.0", Duration::from_secs(2)).unwrap();
    client
        .synthesize_prepared(&input, &AtomicBool::new(false))
        .await
        .unwrap();
    let requests = mock.await.unwrap();
    let text = requests[0]
        .url
        .query_pairs()
        .find(|(key, _)| key == "text")
        .unwrap()
        .1;
    assert_eq!(text, input.resolved().text);
    let client =
        VoicevoxClient::new("http://127.0.0.1:1", "different", Duration::from_secs(2)).unwrap();
    let error = client
        .synthesize_prepared(&input, &AtomicBool::new(false))
        .await
        .err()
        .unwrap();
    assert!(matches!(error, TransportError::Configuration(_)));
}
