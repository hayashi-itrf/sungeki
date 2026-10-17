use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/speech.rs"]
mod setup;
use setup::*;
fn fixture() -> Project {
    let mut p = apply_batch(&Project::empty(id(1), "dialogue".into()), &commands()).unwrap();
    p.scenes.clear();
    p.clips[0].scene_id = None;
    let mut second = character();
    second.id = id(20);
    second.name = "second".into();
    second.credit = "second voice".into();
    p.characters.push(second);
    p.assets.push(Asset {
        id: id(30),
        name: "original".into(),
        source: None,
        credit: "original audio".into(),
        locator: AssetLocator::Absolute {
            path: "/missing/audio.wav".into(),
        },
        content_hash: Some("a".repeat(64)),
        probe: AssetProbe::Audio(AudioProbe {
            encoding: None,
            sha256: "a".repeat(64),
            duration_us: TimeUs(1_000_000),
            sample_rate: 24000,
            channels: 1,
            frames: 24000,
            byte_length: 48044,
        }),
    });
    if let ClipContent::Speech { audio, .. } = &mut p.clips[0].content {
        *audio = Some(SpeechAudio {
            irodori_source: None,
            qwen_source: None,
            sbv2_source: None,
            asset_id: id(30),
            input_fingerprint: "b".repeat(64),
            engine_version: "test".into(),
            lip_sync: None,
        });
    }
    let mut second_clip = p.clips[0].clone();
    second_clip.id = id(7);
    second_clip.start_us = TimeUs(3_000_000);
    if let ClipContent::Speech { character_id, .. } = &mut second_clip.content {
        *character_id = id(20);
    }
    p.clips.push(second_clip);
    p.assets.push(Asset {
        id: id(31),
        name: "image".into(),
        source: None,
        credit: "original image".into(),
        locator: AssetLocator::Absolute {
            path: "/missing/image.png".into(),
        },
        content_hash: Some("c".repeat(64)),
        probe: AssetProbe::Image(ImageProbe {
            sha256: "c".repeat(64),
            format: ImageFormat::Png,
            width: 100,
            height: 200,
            orientation: 1,
            has_alpha: true,
            byte_length: 100,
        }),
    });
    p.tracks.push(Track {
        id: id(12),
        name: "visual".into(),
        kind: TrackKind::Visual,
        visible: true,
        muted: false,
    });
    for n in [8, 9, 10] {
        let mut c = p.clips[0].clone();
        c.id = id(n);
        c.track_id = id(12);
        c.content = ClipContent::Image { asset_id: id(31) };
        p.clips.push(c);
    }
    let mut bgm = p.clips[0].clone();
    bgm.id = id(11);
    bgm.content = ClipContent::Audio {
        asset_id: id(30),
        playback: AudioPlayback {
            volume_points: vec![],
            source_start_us: TimeUs(0),
            source_duration_us: TimeUs(1_000_000),
            looped: true,
            gain_db: -10.,
            fade_in_us: TimeUs(0),
            fade_out_us: TimeUs(0),
            ducking: None,
        },
    };
    p.clips.push(bgm);
    validate(&p).unwrap();
    p
}
fn template() -> DialogueTemplate {
    DialogueTemplate {
        preset_id: "dialogue-wide-v1".into(),
        title: "dialogue".into(),
        speech_clip_ids: vec![id(4), id(7)],
        participants: vec![
            DialogueParticipant {
                character_id: id(2),
                standing_clip_id: id(8),
            },
            DialogueParticipant {
                character_id: id(20),
                standing_clip_id: id(9),
            },
        ],
        background_clip_id: id(10),
        bgm_clip_id: id(11),
        start_us: TimeUs(0),
        gap_us: TimeUs(350_000),
        scene_id: id(100),
        track_ids: (101..107).map(id).collect(),
        title_clip_id: id(107),
        credits_clip_id: id(108),
        title_backdrop_id: id(109),
        credits_backdrop_id: id(110),
    }
}
#[test]
fn fits_all_media_preserves_audio_and_is_one_undo() {
    let mut p = fixture();
    p.clips
        .iter_mut()
        .find(|c| c.id == id(8))
        .unwrap()
        .transform
        .flip_x = Some(true);
    let mut h = History::new(p.clone()).unwrap();
    h.apply(
        0,
        &[Command::ApplyDialogueTemplate {
            template: template(),
        }],
    )
    .unwrap();
    let next = h.project().clone();
    assert_eq!(next.scenes[0].duration_us, TimeUs(8_850_000));
    assert_eq!(
        next.clips
            .iter()
            .find(|c| c.id == id(8))
            .unwrap()
            .transform
            .flip_x,
        Some(true)
    );
    assert_eq!(next.clips[0].start_us, TimeUs(3_000_000));
    assert_eq!(next.clips[1].start_us, TimeUs(4_350_000));
    assert_eq!(next.clips[0].content, p.clips[0].content);
    assert!(next.clips.iter().all(|c| c.scene_id == Some(id(100))));
    let bg = next.clips.iter().find(|c| c.id == id(10)).unwrap();
    assert_eq!(bg.duration_us, next.scenes[0].duration_us);
    assert_eq!(bg.transform.width, 1920.);
    assert_eq!(bg.transform.height, 3840.);
    let credits = next.clips.iter().find(|c| c.id == id(108)).unwrap();
    assert!(
        matches!(&credits.content,ClipContent::Text{text,..} if text.contains("original image") && text.contains("second voice"))
    );
    assert_eq!(
        serde_json::from_str::<Project>(&serde_json::to_string(&next).unwrap()).unwrap(),
        next
    );
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &next);
}
#[test]
fn rejects_incomplete_or_conflicting_template_atomically() {
    for mode in 0..7 {
        let mut p = fixture();
        let mut t = template();
        match mode {
            0 => t.track_ids[1] = t.track_ids[0].clone(),
            1 => t.participants[1].character_id = id(2),
            2 => p.assets[1].credit.clear(),
            3 => t.start_us = TimeUs(MAX_TIME_US),
            4 => {
                let mut c = p.clips[0].clone();
                c.id = id(99);
                p.clips.push(c);
            }
            5 => {
                if let ClipContent::Speech { audio, .. } = &mut p.clips[0].content {
                    *audio = None;
                }
            }
            _ => t.preset_id = "unknown".into(),
        }
        let mut h = History::new(p.clone()).unwrap();
        assert!(
            h.apply(0, &[Command::ApplyDialogueTemplate { template: t }])
                .is_err()
        );
        assert_eq!(h.project(), &p);
    }
}

#[test]
fn batch_references_feed_new_people_and_clips_into_dialogue_template() {
    use std::collections::BTreeMap;
    let original = fixture();
    let mut base = original.clone();
    base.characters.clear();
    base.clips.clear();
    let mut refs = BTreeMap::new();
    let mut commands = Vec::new();
    for mut character in original.characters.clone() {
        let name = format!("person{}", refs.len());
        refs.insert(name.clone(), character.id.clone());
        character.id = format!("${name}");
        commands.push(Command::SetCharacter { character });
    }
    for mut clip in original.clips.clone() {
        let name = format!("clip{}", refs.len());
        refs.insert(name.clone(), clip.id.clone());
        clip.id = format!("${name}");
        commands.push(Command::AddClip { clip });
    }
    let reference = |id: &str| {
        format!(
            "${}",
            refs.iter()
                .find(|(_, value)| value.as_str() == id)
                .unwrap()
                .0
        )
    };
    let mut t = template();
    for id in &mut t.speech_clip_ids {
        *id = reference(id);
    }
    for participant in &mut t.participants {
        participant.character_id = reference(&participant.character_id);
        participant.standing_clip_id = reference(&participant.standing_clip_id);
    }
    t.background_clip_id = reference(&t.background_clip_id);
    t.bgm_clip_id = reference(&t.bgm_clip_id);
    t.title = "$person0 remains literal".into();
    commands.push(Command::ApplyDialogueTemplate { template: t });
    let plan = plan_batch_references(&base, &commands, &refs).unwrap();
    let mut normal = template();
    normal.title = "$person0 remains literal".into();
    let expected = apply_batch(
        &original,
        &[Command::ApplyDialogueTemplate { template: normal }],
    )
    .unwrap();
    assert_eq!(apply_batch(&base, &plan.commands).unwrap(), expected);
    if let Command::ApplyDialogueTemplate { template } = commands.last_mut().unwrap() {
        template.participants[0].character_id = "$missing".into();
    }
    assert!(plan_batch_references(&base, &commands, &refs).is_err());
}
