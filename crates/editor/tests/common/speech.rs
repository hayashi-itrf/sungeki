use sungeki_editor::*;
use sungeki_project::*;
pub fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
pub fn character() -> Character {
    Character {
        id: id(2),
        name: "話者".into(),
        credit: "VOICEVOX:検証話者".into(),
        subtitle_style: SubtitleStyle {
            layout: None,
            outline: None,
            font_size: 64.0,
            color: "#123456".into(),
        },
        voice: Voice::Voicevox {
            engine_id: "test-engine".into(),
            core_version: "0.16.0".into(),
            settings: VoicevoxSettings {
                style_id: 1,
                speed_scale: 1.0,
                pitch_scale: 0.0,
                intonation_scale: 1.0,
                volume_scale: 1.0,
                pre_phoneme_length: 0.1,
                post_phoneme_length: 0.1,
                pause_length: None,
                pause_length_scale: 1.0,
                output_sampling_rate: 24000,
                output_stereo: false,
                enable_interrogative_upspeak: true,
            },
        },
    }
}
pub fn commands() -> Vec<Command> {
    vec![
        Command::SetCharacter {
            character: character(),
        },
        Command::AddTrack {
            track: Track {
                id: id(3),
                name: "発話".into(),
                visible: true,
                muted: false,
                kind: TrackKind::Audio,
            },
        },
        Command::AddScene {
            scene: Scene {
                fade: None,
                id: id(5),
                name: "導入".into(),
                start_us: TimeUs(0),
                duration_us: TimeUs(5_000_000),
            },
        },
        Command::AddClip {
            clip: Clip {
                id: id(4),
                track_id: id(3),
                scene_id: Some(id(5)),
                start_us: TimeUs(0),
                duration_us: TimeUs(2_000_000),
                transform: Transform {
                    motion: None,
                    x: 10.0,
                    y: 20.0,
                    width: 100.0,
                    height: 50.0,
                    opacity: 1.0,
                    fade: None,
                    flip_x: None,
                },
                content: ClipContent::Speech {
                    gain_db: None,
                    audio: None,
                    character_id: id(2),
                    subtitle_text: "字幕".into(),
                    spoken_text: Some("よみ".into()),
                },
            },
        },
    ]
}
