use serde_json::{Value, json};
use sungeki_speech::voicevox::{AudioQuery, VoicevoxSettings, prepare_synthesis_query};

fn settings() -> VoicevoxSettings {
    VoicevoxSettings {
        style_id: 42,
        speed_scale: 1.2,
        pitch_scale: -0.1,
        intonation_scale: 0.8,
        volume_scale: 0.5,
        pre_phoneme_length: 0.2,
        post_phoneme_length: 0.3,
        pause_length: Some(0.4),
        pause_length_scale: 1.3,
        output_sampling_rate: 48000,
        output_stereo: true,
        enable_interrogative_upspeak: true,
    }
}
fn response() -> Value {
    json!({
        "accent_phrases": [{
            "moras": [{"text":"ア", "consonant":null, "consonant_length":null,
                "vowel":"a", "vowel_length":0.1, "pitch":5.8}],
            "accent":1, "pause_mora": {"text":"、", "vowel":"pau",
                "vowel_length":0.2, "pitch":0}, "is_interrogative":true
        }],
        "speedScale":1.0, "pitchScale":0.0, "intonationScale":1.0, "volumeScale":1.0,
        "prePhonemeLength":0.1, "postPhonemeLength":0.1, "pauseLength":null,
        "pauseLengthScale":1.0, "outputSamplingRate":24000, "outputStereo":false,
        "kana":"ア'？"
    })
}
fn prepare(value: &Value) -> Result<AudioQuery, sungeki_speech::voicevox::VoicevoxError> {
    prepare_synthesis_query(&serde_json::to_vec(value).unwrap(), &settings())
}
#[test]
fn applies_all_controls_without_losing_pronunciation_or_mutating_response() {
    let response = response();
    let original = response.clone();
    let query = prepare(&response).unwrap();
    let body = serde_json::to_value(&query).unwrap();
    assert_eq!(
        body["accent_phrases"],
        json!([{
            "moras": [{"text":"ア","consonant":null,"consonant_length":null,
                "vowel":"a","vowel_length":0.1,"pitch":5.8}], "accent":1,
            "pause_mora":{"text":"、","consonant":null,"consonant_length":null,
                "vowel":"pau","vowel_length":0.2,"pitch":0.0}, "is_interrogative":true
        }])
    );
    for (key, expected) in [
        ("speedScale", json!(1.2)),
        ("pitchScale", json!(-0.1)),
        ("intonationScale", json!(0.8)),
        ("volumeScale", json!(0.5)),
        ("prePhonemeLength", json!(0.2)),
        ("postPhonemeLength", json!(0.3)),
        ("pauseLength", json!(0.4)),
        ("pauseLengthScale", json!(1.3)),
        ("outputSamplingRate", json!(48000)),
        ("outputStereo", json!(true)),
        ("kana", json!("ア'？")),
    ] {
        assert_eq!(body[key], expected, "{key}");
    }
    assert!(body.get("style_id").is_none());
    assert!(body.get("enable_interrogative_upspeak").is_none());
    assert_eq!(response, original);
    let mut controls = settings();
    controls.pause_length = None;
    let query =
        prepare_synthesis_query(&serde_json::to_vec(&response).unwrap(), &controls).unwrap();
    assert_eq!(query.pause_length, None);
}
#[test]
fn rejects_bad_response_even_if_a_control_would_overwrite_it() {
    for (pointer, invalid) in [
        ("/speedScale", json!(0)),
        ("/volumeScale", json!(-1)),
        ("/outputSamplingRate", json!(0)),
        ("/outputSamplingRate", json!(2.5)),
        ("/accent_phrases/0/accent", json!(0)),
        ("/accent_phrases/0/accent", json!(2)),
        ("/accent_phrases/0/moras/0/vowel_length", json!(-1)),
        ("/accent_phrases/0/pause_mora/vowel_length", json!(-1)),
        ("/accent_phrases/0/moras/0/consonant_length", json!(-1)),
    ] {
        let mut data = response();
        *data.pointer_mut(pointer).unwrap() = invalid;
        assert!(prepare(&data).is_err(), "{pointer}");
    }
    let mut missing = response();
    missing.as_object_mut().unwrap().remove("speedScale");
    assert!(prepare(&missing).is_err());
    assert!(prepare_synthesis_query(b"not JSON", &settings()).is_err());
    assert!(prepare_synthesis_query(&vec![b' '; 8 * 1024 * 1024 + 1], &settings()).is_err());
}
#[test]
fn optional_engine_fields_follow_official_defaults() {
    let mut data = response();
    for name in ["pauseLength", "pauseLengthScale", "kana"] {
        data.as_object_mut().unwrap().remove(name);
    }
    data["accent_phrases"][0]
        .as_object_mut()
        .unwrap()
        .remove("is_interrogative");
    data["future_engine_extension"] = json!(true);
    let query: AudioQuery = serde_json::from_value(data).unwrap();
    query.validate().unwrap();
    assert_eq!(query.pause_length, None);
    assert_eq!(query.pause_length_scale, 1.0);
    assert_eq!(query.kana, None);
    assert!(!query.accent_phrases[0].is_interrogative);
}
#[test]
fn rejects_nonfinite_controls_and_invalid_bounds_before_serialization() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0] {
        let mut s = settings();
        s.speed_scale = value;
        assert!(prepare_synthesis_query(b"{}", &s).is_err());
    }
    for field in 0..7 {
        let mut s = settings();
        let target = match field {
            0 => &mut s.pitch_scale,
            1 => &mut s.intonation_scale,
            2 => &mut s.volume_scale,
            3 => &mut s.pre_phoneme_length,
            4 => &mut s.post_phoneme_length,
            5 => &mut s.pause_length_scale,
            _ => s.pause_length.as_mut().unwrap(),
        };
        *target = f64::NAN;
        assert!(s.validate().is_err());
    }
    let mut s = settings();
    s.volume_scale = 0.0;
    s.pause_length = Some(0.0);
    s.pause_length_scale = 0.0;
    s.validate().unwrap();
    let mut data = serde_json::to_value(settings()).unwrap();
    data["unknown_legacy_setting"] = json!(true);
    assert!(serde_json::from_value::<VoicevoxSettings>(data).is_err());
}

#[test]
fn received_audio_is_verified_against_the_synthesis_query() {
    use std::io::Cursor;
    use sungeki_speech::voicevox::VoicevoxResponse;
    let query = prepare(&response()).unwrap();
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut bytes, spec).unwrap();
        for _ in 0..960 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();
    }
    let result = VoicevoxResponse {
        query: query.clone(),
        unverified_wav: bytes.into_inner(),
    }
    .verify()
    .unwrap();
    assert_eq!(result.audio.info().frames, 480);
    assert_eq!(result.audio.info().duration_us, 10000);
    assert_eq!(result.query, query);
    let mut wrong = query;
    wrong.output_stereo = false;
    let result = VoicevoxResponse {
        query: wrong,
        unverified_wav: result.audio.bytes().to_vec(),
    }
    .verify();
    assert!(matches!(
        result,
        Err(sungeki_media::WavError::FormatMismatch)
    ));
}
