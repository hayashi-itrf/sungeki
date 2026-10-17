#[path = "irodori/common.rs"]
mod common;
use sungeki_project::{ClipContent, Project, Voice};
use sungeki_speech::irodori::{
    Catalog, IrodoriJobInput, ModelCapabilities, PreparedIrodoriClip, ReferenceKind,
    RegisteredVoice,
};

fn fixture() -> (Project, String, Catalog) {
    let mut project: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let clip = project
        .clips
        .iter_mut()
        .find(|c| matches!(c.content, ClipContent::Speech { .. }))
        .unwrap();
    let id = clip.id.clone();
    let ClipContent::Speech {
        character_id,
        audio,
        ..
    } = &mut clip.content
    else {
        unreachable!()
    };
    *audio = None;
    let source = common::source();
    project
        .characters
        .iter_mut()
        .find(|c| c.id == *character_id)
        .unwrap()
        .voice = Voice::Irodori {
        settings: source.settings,
        identity: source.identity,
    };
    let catalog = Catalog::new(
        ModelCapabilities {
            checkpoint: "model".into(),
            speaker_conditioning: true,
            caption_conditioning: true,
            sample_rate: 24000,
            channels: 1,
        },
        vec![RegisteredVoice {
            id: "registered".into(),
            name: "登録音声".into(),
            reference: ReferenceKind::Audio,
            caption: Some("落ち着いた声".into()),
        }],
    )
    .unwrap();
    (project, id, catalog)
}

#[test]
fn capture_survives_roundtrip_and_owns_project_and_selection() {
    let (mut project, id, catalog) = fixture();
    let job = IrodoriJobInput::new(7, &project, &id, &common::source().identity, &catalog)
        .unwrap()
        .for_profile("irodori-local")
        .unwrap()
        .for_parent("00000000-0000-4000-8000-000000000001")
        .unwrap();
    let bytes = serde_json::to_vec(&job).unwrap();
    let restored: IrodoriJobInput = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored.input_hash(), job.input_hash());
    assert_eq!(restored.source_revision(), 7);
    assert_eq!(restored.clip_id(), id);
    assert_eq!(restored.profile_id(), Some("irodori-local"));
    assert!(restored.matches_project(&project).unwrap());
    let prepared = restored.prepared().unwrap();
    let selected = prepared.audio().selection().unwrap();
    assert_eq!(selected.caption(), Some("落ち着いた声"));
    assert_eq!(selected.reference(), ReferenceKind::Audio);
    project.title = "合成に無関係な改名".into();
    assert!(restored.matches_project(&project).unwrap());
    project
        .clips
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .transform
        .x += 1.;
    assert!(!restored.matches_project(&project).unwrap());
    assert_eq!(serde_json::to_vec(&job).unwrap(), bytes);
}

#[test]
fn changed_text_seed_dictionary_deleted_target_and_runtime_rejected() {
    let (project, id, catalog) = fixture();
    let identity = common::source().identity;
    let input = PreparedIrodoriClip::new(&project, &id, &catalog, &identity).unwrap();
    for mode in 0..5 {
        let mut changed = project.clone();
        match mode {
            0 => {
                let ClipContent::Speech { subtitle_text, .. } = &mut changed
                    .clips
                    .iter_mut()
                    .find(|c| c.id == id)
                    .unwrap()
                    .content
                else {
                    unreachable!()
                };
                *subtitle_text = "変更後の文章".into();
            }
            1 => {
                let Voice::Irodori { settings, .. } = &mut changed
                    .characters
                    .iter_mut()
                    .find(|c| c.id == input.character().id)
                    .unwrap()
                    .voice
                else {
                    unreachable!()
                };
                settings.seed += 1;
            }
            2 => changed
                .pronunciation_dictionary
                .push(sungeki_project::PronunciationEntry {
                    written: "追加語".into(),
                    spoken: "ついかご".into(),
                }),
            3 => changed.clips.retain(|c| c.id != id),
            _ => changed.id = "00000000-0000-4000-8000-000000000002".into(),
        }
        assert!(!input.matches(&changed, &catalog, &identity), "{mode}");
    }
    let mut different = identity.clone();
    different.codec_manifest_sha256 = "f".repeat(64);
    assert!(!input.matches(&project, &catalog, &different));
    let changed_catalog = Catalog::new(
        catalog.model().clone(),
        vec![RegisteredVoice {
            id: "registered".into(),
            name: "登録音声".into(),
            reference: ReferenceKind::Latent,
            caption: None,
        }],
    )
    .unwrap();
    assert!(!input.matches(&project, &changed_catalog, &identity));
    assert!(PreparedIrodoriClip::new(&project, "missing", &catalog, &identity).is_err());
}

#[test]
fn stored_input_tampering_and_invalid_binding_rejected() {
    let (project, id, catalog) = fixture();
    let job = IrodoriJobInput::new(7, &project, &id, &common::source().identity, &catalog).unwrap();
    let original = serde_json::to_value(&job).unwrap();
    for mode in 0..5 {
        let mut changed = original.clone();
        match mode {
            0 => changed["capture"]["source_revision"] = 8.into(),
            1 => changed["capture"]["format_version"] = 2.into(),
            2 => changed["capture"]["voices"][0]["reference"] = "latent".into(),
            3 => changed["capture"]["endpoint"] = "http://legacy".into(),
            _ => changed["input_hash"] = "a".repeat(64).into(),
        }
        assert!(
            serde_json::from_value::<IrodoriJobInput>(changed).is_err(),
            "{mode}"
        );
    }
    assert!(job.clone().for_profile("../escape").is_err());
    assert!(job.clone().for_parent("bad-parent").is_err());
    let bound = job.for_profile("local").unwrap();
    assert_eq!(
        bound.clone().for_profile("local").unwrap().input_hash(),
        bound.input_hash()
    );
    assert!(bound.for_profile("other").is_err());
}
