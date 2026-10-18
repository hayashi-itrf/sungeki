use sungeki_project::{ClipContent, LocalVoiceSettings, Project, Voice};
use sungeki_speech::local_http::{LocalSpeechJobInput, PreparedLocalInput};
fn fixture() -> (Project, String) {
    let mut p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let clip = p
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Speech { .. }))
        .unwrap();
    let id = clip.id.clone();
    let ClipContent::Speech { character_id, .. } = &clip.content else {
        unreachable!()
    };
    let cid = character_id.clone();
    p.characters.iter_mut().find(|c| c.id == cid).unwrap().voice = Voice::LocalHttp {
        settings: LocalVoiceSettings::Coeiroink {
            speaker_uuid: "3c37646f-3881-5374-2a83-149267990abc".into(),
            style_id: 0,
            speed_scale: 1.,
            pitch_scale: 0.,
            intonation_scale: 1.,
            volume_scale: 1.,
            pre_phoneme_length: 0.1,
            post_phoneme_length: 0.1,
        },
    };
    (p, id)
}
#[test]
fn durable_capture_rejects_tampering_and_tracks_text_and_tuning_changes() {
    let (p, id) = fixture();
    let i =
        LocalSpeechJobInput::new(1, &p, &id, "http://127.0.0.1:50032", "2.13.0@catalog").unwrap();
    let wire = serde_json::to_value(&i).unwrap();
    let round: LocalSpeechJobInput = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(round.input_hash(), i.input_hash());
    let mut broken = wire;
    broken["capture"]["engine_version"] = serde_json::json!("other");
    assert!(serde_json::from_value::<LocalSpeechJobInput>(broken).is_err());
    let original = i.prepared().unwrap();
    assert!(original.matches(&p));
    let mut changed = p.clone();
    if let ClipContent::Speech { subtitle_text, .. } = &mut changed
        .clips
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .content
    {
        *subtitle_text = "読みが変わる".into();
    }
    assert!(!original.matches(&changed));
    assert_ne!(
        original.fingerprint(),
        PreparedLocalInput::new(&changed, &id, i.engine_version())
            .unwrap()
            .fingerprint()
    );
    for c in &mut changed.characters {
        if let Voice::LocalHttp {
            settings: LocalVoiceSettings::Coeiroink { speed_scale, .. },
        } = &mut c.voice
        {
            *speed_scale = 1.5;
        }
    }
    assert!(!i.matches_project(&changed).unwrap());
    assert!(original.verify_audio(vec![0; 100]).is_err());
}
#[test]
fn persisted_project_keeps_local_voice_selection() {
    let (p, _) = fixture();
    let copy: Project = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
    assert_eq!(p, copy);
}
mod http_support;
use http_support::{Reply, server};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use sungeki_speech::{local_http::LocalSpeechClient, voicevox::TransportError};
#[tokio::test]
async fn coeiroink_request_uses_v2_fields_and_cancellation_drains_the_response() {
    let (p, id) = fixture();
    let input = PreparedLocalInput::new(&p, &id, "test").unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let mut reply = Reply::ok(vec![1, 2, 3]);
    reply.cancel = Some(cancel.clone());
    reply.body_delay = Duration::from_millis(50);
    let (url, worker) = server(vec![reply]).await;
    let client =
        LocalSpeechClient::new(&url, input.settings().engine(), Duration::from_secs(2)).unwrap();
    let start = Instant::now();
    assert!(matches!(
        client.synthesize(&input, &cancel).await,
        Err(TransportError::Cancelled)
    ));
    assert!(start.elapsed() >= Duration::from_millis(50));
    let requests = worker.await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.path(), "/v1/synthesis");
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["speakerUuid"], input.settings().selection().0);
    assert_eq!(body["text"], input.text());
    assert_eq!(body["outputSamplingRate"], 44100);
    assert_eq!(body["processingAlgorithm"], "RESAMPLING");
}
#[tokio::test]
async fn synthesis_timeout_is_unsettled_and_never_automatically_retried() {
    let (p, id) = fixture();
    let input = PreparedLocalInput::new(&p, &id, "test").unwrap();
    let mut reply = Reply::ok(vec![1, 2, 3]);
    reply.body_delay = Duration::from_millis(150);
    let (url, worker) = server(vec![reply]).await;
    let client =
        LocalSpeechClient::new(&url, input.settings().engine(), Duration::from_millis(30)).unwrap();
    let err = client
        .synthesize(&input, &AtomicBool::new(false))
        .await
        .unwrap_err();
    assert!(err.requires_engine_settlement());
    assert_eq!(worker.await.unwrap().len(), 1);
}
#[tokio::test]
async fn aivis_preserves_text_and_passes_its_own_controls_and_signed_style() {
    let (mut p, id) = fixture();
    for c in &mut p.characters {
        if matches!(c.voice, Voice::LocalHttp { .. }) {
            c.voice = Voice::LocalHttp {
                settings: LocalVoiceSettings::AivisSpeech {
                    speaker_uuid: "5680ac39-43c9-487a-bc3e-018c0d29cc38".into(),
                    style_id: -12,
                    speed_scale: 1.2,
                    pitch_scale: 0.,
                    emotion_scale: 1.4,
                    tempo_dynamics_scale: 0.8,
                    volume_scale: 1.,
                    pre_phoneme_length: 0.1,
                    post_phoneme_length: 0.2,
                },
            };
        }
    }
    let input = PreparedLocalInput::new(&p, &id, "test").unwrap();
    let (url, worker) = server(vec![
        Reply::ok(http_support::query()),
        Reply::ok(vec![1, 2, 3]),
    ])
    .await;
    let client =
        LocalSpeechClient::new(&url, input.settings().engine(), Duration::from_secs(2)).unwrap();
    assert_eq!(
        client
            .synthesize(&input, &AtomicBool::new(false))
            .await
            .unwrap(),
        vec![1, 2, 3]
    );
    let requests = worker.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0]
            .url
            .query_pairs()
            .any(|(k, v)| k == "speaker" && v == "-12")
    );
    let body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(body["kana"], input.text());
    assert_eq!(body["intonationScale"], 1.4);
    assert_eq!(body["tempoDynamicsScale"], 0.8);
    assert_eq!(body["outputSamplingRate"], 44100);
    assert_eq!(body["outputStereo"], false);
}
