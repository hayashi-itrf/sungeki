#[path = "irodori/common.rs"]
mod common;
use sungeki_speech::irodori::{
    Catalog, ModelCapabilities, PreparedIrodoriInput, ReferenceKind, RegisteredVoice,
};

fn model(speaker: bool, caption: bool) -> ModelCapabilities {
    ModelCapabilities {
        checkpoint: "model".into(),
        speaker_conditioning: speaker,
        caption_conditioning: caption,
        sample_rate: 24000,
        channels: 1,
    }
}
fn voice(reference: ReferenceKind, caption: Option<&str>) -> RegisteredVoice {
    RegisteredVoice {
        id: "registered".into(),
        name: "話者".into(),
        reference,
        caption: caption.map(str::to_owned),
    }
}
#[test]
fn incompatible_conditions_and_unknown_voice_are_rejected() {
    let input = common::source();
    let reference =
        Catalog::new(model(false, false), vec![voice(ReferenceKind::Audio, None)]).unwrap();
    assert!(reference.select(&input.settings, &input.identity).is_err());
    let caption = Catalog::new(
        model(true, false),
        vec![voice(ReferenceKind::Audio, Some("落ち着いた声"))],
    )
    .unwrap();
    assert!(caption.select(&input.settings, &input.identity).is_err());
    let valid = Catalog::new(model(true, false), vec![voice(ReferenceKind::Latent, None)]).unwrap();
    let mut settings = input.settings.clone();
    settings.voice_id = "unknown".into();
    assert!(valid.select(&settings, &input.identity).is_err());
    let mut identity = input.identity.clone();
    identity.checkpoint = "other".into();
    assert!(valid.select(&input.settings, &identity).is_err());
    identity = input.identity.clone();
    identity.sample_rate = 48000;
    assert!(valid.select(&input.settings, &identity).is_err());
}
#[test]
fn caption_selection_and_explicit_no_reference_are_captured() {
    let input = common::source();
    let catalog = Catalog::new(
        model(true, true),
        vec![voice(ReferenceKind::None, Some("登録された説明"))],
    )
    .unwrap();
    let selected = catalog.select(&input.settings, &input.identity).unwrap();
    assert!(selected.no_reference());
    assert_eq!(selected.caption(), Some("登録された説明"));
    let prepared = PreparedIrodoriInput::from_selected("確認", None, &[], &selected).unwrap();
    assert_eq!(
        prepared.selection().unwrap().caption(),
        Some("登録された説明")
    );
    let offline = PreparedIrodoriInput::new("確認", None, &[], prepared.audio_source()).unwrap();
    assert!(offline.selection().is_none());
    assert_eq!(offline.fingerprint(), prepared.fingerprint());
    let mut settings = input.settings.clone();
    settings.caption = Some("明示した説明".into());
    let override_voice = catalog.select(&settings, &input.identity).unwrap();
    assert_eq!(override_voice.caption(), Some("明示した説明"));
    assert_ne!(
        prepared.fingerprint(),
        PreparedIrodoriInput::from_selected("確認", None, &[], &override_voice)
            .unwrap()
            .fingerprint()
    );
}
#[test]
fn unrelated_voice_and_display_name_do_not_change_selected_metadata() {
    let input = common::source();
    let first = voice(ReferenceKind::Audio, None);
    let mut other = first.clone();
    other.id = "other".into();
    let a = Catalog::new(model(true, true), vec![first.clone(), other.clone()]).unwrap();
    let mut renamed = first.clone();
    renamed.name = "表示名変更".into();
    other.caption = Some("変更".into());
    let b = Catalog::new(model(true, true), vec![other, renamed]).unwrap();
    assert_eq!(
        a.select(&input.settings, &input.identity)
            .unwrap()
            .audio_source(),
        b.select(&input.settings, &input.identity)
            .unwrap()
            .audio_source()
    );
    let mut changed = first;
    changed.reference = ReferenceKind::Latent;
    let c = Catalog::new(model(true, true), vec![changed]).unwrap();
    assert_ne!(
        a.select(&input.settings, &input.identity)
            .unwrap()
            .audio_source()
            .metadata_sha256,
        c.select(&input.settings, &input.identity)
            .unwrap()
            .audio_source()
            .metadata_sha256
    );
}
#[test]
fn malformed_and_duplicate_catalog_entries_are_rejected() {
    let v = voice(ReferenceKind::None, None);
    assert!(Catalog::new(model(true, true), vec![v.clone(), v.clone()]).is_err());
    assert!(Catalog::new(model(true, true), vec![]).is_err());
    let mut bad = v;
    bad.caption = Some(" ".into());
    assert!(Catalog::new(model(true, true), vec![bad]).is_err());
    assert!(serde_json::from_str::<RegisteredVoice>(r#"{"id":"x","name":"x","reference":"audio","caption":null,"ref_audio_path":"/unexpected.wav"}"#).is_err());
}
