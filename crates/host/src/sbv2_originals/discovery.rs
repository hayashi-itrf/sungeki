//! Read-only registration metadata derived from explicitly configured originals.
use super::{configuration::Profile, inspect, invalid};
use std::sync::atomic::AtomicBool;
use sungeki_project::{Sbv2Identity, Sbv2Language, Sbv2Settings};
use sungeki_protocol::{ApiError, Sbv2ProfileDetails, Sbv2Speaker};
use sungeki_speech::sbv2::{Catalog, ModelConfig};

pub(crate) fn details(
    profile: &Profile,
    cancel: &AtomicBool,
) -> Result<Sbv2ProfileDetails, ApiError> {
    let catalog = Catalog::parse(profile.catalog.clone()).map_err(invalid)?;
    // A profile owns one model original. Do not inspect unrelated metadata paths.
    let mut candidates = Vec::new();
    for (&model_id, model) in catalog.models() {
        let settings = Sbv2Settings {
            model_id,
            speaker_id: *model
                .spk2id
                .values()
                .min()
                .ok_or_else(|| invalid("話者がありません"))?,
            language: Sbv2Language::JP,
            style: if model.style2id.contains_key("Neutral") {
                "Neutral".into()
            } else {
                model
                    .style2id
                    .keys()
                    .next()
                    .ok_or_else(|| invalid("スタイルがありません"))?
                    .clone()
            },
            style_weight: 1.,
            length: 1.,
            sdp_ratio: 0.2,
            noise: 0.6,
            noisew: 0.8,
            auto_split: true,
            split_interval: 0.5,
        };
        if profile
            .binding
            .validate_catalog(&profile.binding.endpoint, &catalog, &settings)
            .is_ok()
        {
            candidates.push(settings);
        }
    }
    if candidates.len() != 1 {
        return Err(invalid(
            "起動設定のモデル原本に対応するモデルを一つに特定できません",
        ));
    }
    let settings = candidates.pop().unwrap();
    let engine_revision = profile.binding.engine.revision(cancel)?;
    let measured = inspect(&profile.binding.originals, cancel)?;
    let config = ModelConfig::parse(measured.config_text.as_bytes()).map_err(invalid)?;
    config
        .expected_audio(&catalog, &settings)
        .map_err(invalid)?;
    let identity = Sbv2Identity {
        engine_revision,
        model_sha256: measured.model_sha256,
        config_sha256: measured.config_sha256,
        style_vectors_sha256: measured.style_vectors_sha256,
        text_encoder_fingerprint: measured.text_encoder_fingerprint,
    };
    identity.validate().map_err(invalid)?;
    let model = &catalog.models()[&settings.model_id];
    let mut speakers: Vec<_> = model
        .spk2id
        .iter()
        .map(|(name, &id)| Sbv2Speaker {
            id,
            name: name.clone(),
        })
        .collect();
    speakers.sort_by_key(|s| s.id);
    Ok(Sbv2ProfileDetails {
        profile_id: profile.id.clone(),
        name: profile.name.clone(),
        identity,
        speakers,
        styles: model.style2id.keys().cloned().collect(),
        languages: if config.version().ends_with("JP-Extra") {
            vec![Sbv2Language::JP]
        } else {
            vec![Sbv2Language::JP, Sbv2Language::EN, Sbv2Language::ZH]
        },
        default_settings: settings,
    })
}
