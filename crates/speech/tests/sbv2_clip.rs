use serde_json::json;
use sungeki_project::*;
use sungeki_speech::sbv2::{Catalog, ModelConfig, PreparedSbv2Clip};
mod sbv2_target_support;
use sbv2_target_support::{environment, id, project};
#[test]
fn saved_target_stays_immutable_and_rejects_changes_before_apply() {
    let (catalog, config, identity) = environment();
    let original = project(&identity);
    let saved: Project = serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
    let fixed = PreparedSbv2Clip::new(&saved, &id(4), &catalog, &config, &identity).unwrap();
    assert_eq!(fixed.audio().resolved().text, "よみ");
    assert_eq!(fixed.clip(), &original.clips[0]);
    assert_eq!(fixed.character(), &original.characters[0]);
    assert!(fixed.matches(&saved, &catalog, &config, &identity));
    let mut unrelated = saved.clone();
    unrelated.canvas.background = "#112233".into();
    assert!(fixed.matches(&unrelated, &catalog, &config, &identity));
    for mutation in 0..10 {
        let mut changed = saved.clone();
        match mutation {
            0 => changed.clips.clear(),
            1 => changed.id = id(99),
            2 => changed.clips[0].duration_us = TimeUs(2_000_000),
            3 => changed.clips[0].start_us = TimeUs(1),
            4 => changed.characters[0].subtitle_style.font_size = 72.,
            5 => changed.characters.clear(),
            6 => changed.pronunciation_dictionary.push(PronunciationEntry {
                written: "字幕".into(),
                spoken: "じまく".into(),
            }),
            7 => {
                let Voice::Sbv2 { settings, .. } = &mut changed.characters[0].voice else {
                    panic!()
                };
                settings.length = 1.5;
            }
            8 => {
                let ClipContent::Speech { subtitle_text, .. } = &mut changed.clips[0].content
                else {
                    panic!()
                };
                *subtitle_text = "別字幕".into();
            }
            _ => {
                let ClipContent::Speech { spoken_text, .. } = &mut changed.clips[0].content else {
                    panic!()
                };
                *spoken_text = Some("別の読み".into());
            }
        }
        assert!(
            !fixed.matches(&changed, &catalog, &config, &identity),
            "mutation {mutation}"
        );
    }
    assert_eq!(fixed.audio().resolved().text, "よみ");
}
#[test]
fn rejects_runtime_identity_and_model_changes() {
    let (catalog, config, identity) = environment();
    let project = project(&identity);
    let fixed = PreparedSbv2Clip::new(&project, &id(4), &catalog, &config, &identity).unwrap();
    let mut changed = identity.clone();
    changed.model_sha256 = "d".repeat(64);
    assert!(PreparedSbv2Clip::new(&project, &id(4), &catalog, &config, &changed).is_err());
    assert!(!fixed.matches(&project, &catalog, &config, &changed));
    let changed_catalog = Catalog::parse(json!({"0":{"model_path":"/ssd/other",
        "config_path":"/ssd/config","device":"cpu","spk2id":{"声":0},
        "id2spk":{"0":"声"},"style2id":{"Neutral":0}}}))
    .unwrap();
    assert!(!fixed.matches(&project, &changed_catalog, &config, &identity));
    let changed_config = ModelConfig::parse(
        &serde_json::to_vec(&json!({"version":"2.7.0-JP-Extra",
        "data":{"sampling_rate":48000,"spk2id":{"声":0},"num_styles":1,
        "style2id":{"Neutral":0}}}))
        .unwrap(),
    )
    .unwrap();
    assert!(!fixed.matches(&project, &catalog, &changed_config, &identity));
    assert!(PreparedSbv2Clip::new(&project, &id(99), &catalog, &config, &identity).is_err());
}
