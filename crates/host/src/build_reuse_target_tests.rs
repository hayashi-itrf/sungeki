use super::Input;
use crate::{
    build_record::reuse_target::prepare,
    speech_journal_tests::{Fixture, id, seed},
};
use sungeki_project::{ClipContent, Voice};

#[test]
fn all_four_engines_recapture_target_and_preserve_audio_inputs_without_mutating_source() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let originals = vec![
        Input::Voicevox(seed(&mut sessions).1),
        super::sbv2_tests::sbv2(&mut sessions).1,
        super::qwen_tests::qwen(&mut sessions).1,
        super::irodori_tests::irodori(&mut sessions).1,
    ];
    for original in originals {
        let old_parent = id();
        let original = match original {
            Input::Local(_) => unreachable!("this fixture covers the four build-video engines"),
            Input::Voicevox(i) => Input::Voicevox(i.for_parent(&old_parent).unwrap()),
            Input::Sbv2(i) => Input::Sbv2(
                i.for_profile("reuse-test")
                    .unwrap()
                    .for_parent(&old_parent)
                    .unwrap(),
            ),
            Input::Qwen(i) => Input::Qwen(
                i.for_profile("reuse-test")
                    .unwrap()
                    .for_parent(&old_parent)
                    .unwrap(),
            ),
            Input::Irodori(i) => Input::Irodori(
                i.for_profile("reuse-test")
                    .unwrap()
                    .for_parent(&old_parent)
                    .unwrap(),
            ),
        };
        let before = serde_json::to_value(&original).unwrap();
        let mut project = original.project().clone();
        project.id = id();
        project.title = "音声を引き継いだ別の作品".into();
        let clip = project
            .clips
            .iter_mut()
            .find(|c| c.id == original.clip_id())
            .unwrap();
        clip.id = id();
        let clip_id = clip.id.clone();
        let parent = id();
        let target = prepare(&original, 10, &project, &clip_id, &parent).unwrap();
        assert_eq!(target.parent_job_id(), Some(parent.as_str()));
        assert_eq!(target.source_revision(), 10);
        assert_eq!(target.clip_id(), clip_id);
        assert_ne!(target.input_hash(), original.input_hash());
        let encoded = serde_json::to_vec(&target).unwrap();
        let loaded: Input = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(loaded.input_hash(), target.input_hash());
        assert_eq!(
            target.application(&project).unwrap().fingerprint,
            original
                .application(original.project())
                .unwrap()
                .fingerprint
        );
        match &target {
            Input::Local(_) => unreachable!("this fixture covers the four build-video engines"),
            Input::Voicevox(_) => {}
            Input::Sbv2(i) => assert_eq!(i.profile_id(), Some("reuse-test")),
            Input::Qwen(i) => assert_eq!(i.profile_id(), Some("reuse-test")),
            Input::Irodori(i) => assert_eq!(i.profile_id(), Some("reuse-test")),
        }
        let mut changed = project.clone();
        let clip = changed.clips.iter_mut().find(|c| c.id == clip_id).unwrap();
        let ClipContent::Speech { spoken_text, .. } = &mut clip.content else {
            panic!()
        };
        *spoken_text = Some("新しい読み方です".into());
        assert!(prepare(&original, 10, &changed, &clip_id, &parent).is_err());
        let mut changed = project.clone();
        match &mut changed.characters[0].voice {
            Voice::LocalHttp { .. } => unreachable!("this fixture covers the four build-video engines"),
            Voice::Voicevox { settings, .. } => settings.speed_scale = 1.2,
            Voice::Sbv2 { settings, .. } => settings.length = 1.2,
            Voice::Qwen { settings, .. } => settings.speed = 1.2,
            Voice::Irodori { settings, .. } => settings.seed += 1,
        }
        assert!(prepare(&original, 10, &changed, &clip_id, &parent).is_err());
        assert!(prepare(&original, 10, &project, &clip_id, "invalid-parent").is_err());
        assert_eq!(serde_json::to_value(&original).unwrap(), before);
    }
}
