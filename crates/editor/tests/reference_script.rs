use std::collections::BTreeMap;
use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/speech.rs"]
mod setup;
use setup::*;
#[test]
fn newly_created_people_and_tracks_are_usable_in_script_lines() {
    let p = Project::empty(id(1), "script reference".into());
    let mut first = character();
    first.id = "$first".into();
    let mut second = character();
    second.id = "$second".into();
    let track = Track {
        id: "$voice".into(),
        name: "voice".into(),
        kind: TrackKind::Audio,
        visible: false,
        muted: false,
    };
    let commands = vec![
        Command::SetCharacter { character: first },
        Command::SetCharacter { character: second },
        Command::AddTrack { track },
        Command::AppendSpeechScript {
            script: SpeechScript {
                character_id: "$first".into(),
                track_id: "$voice".into(),
                scene_id: None,
                start_us: TimeUs(0),
                provisional_duration_us: TimeUs(1000000),
                gap_us: TimeUs(100000),
                lines: vec![
                    SpeechScriptLine {
                        clip_id: id(10),
                        character_id: None,
                        subtitle_text: "$second".into(),
                        spoken_text: None,
                    },
                    SpeechScriptLine {
                        clip_id: id(11),
                        character_id: Some("$second".into()),
                        subtitle_text: "$first".into(),
                        spoken_text: Some("$voice".into()),
                    },
                ],
            },
        },
    ];
    let ids = BTreeMap::from([
        ("first".into(), id(2)),
        ("second".into(), id(3)),
        ("voice".into(), id(4)),
    ]);
    let plan = plan_batch_references(&p, &commands, &ids).unwrap();
    let result = apply_batch(&p, &plan.commands).unwrap();
    assert_eq!(result.clips.len(), 2);
    assert_eq!(result.clips[1].start_us, TimeUs(1100000));
    assert!(
        matches!(&result.clips[0].content,ClipContent::Speech{character_id,subtitle_text,..} if character_id==&id(2) && subtitle_text=="$second")
    );
    assert!(
        matches!(&result.clips[1].content,ClipContent::Speech{character_id,subtitle_text,spoken_text,..} if character_id==&id(3) && subtitle_text=="$first" && spoken_text.as_deref()==Some("$voice"))
    );
    assert!(result.clips.iter().all(|c| c.track_id == id(4)));
}
