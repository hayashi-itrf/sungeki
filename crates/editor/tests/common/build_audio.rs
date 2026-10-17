use super::setup::id;
use sungeki_project::*;
pub fn result(index: u32, ms: u64) -> (Asset, SpeechAudio) {
    let asset_id = id(200 + index);
    (
        Asset {
            id: asset_id.clone(),
            name: "合成結果".into(),
            source: None,
            credit: "VOICEVOX:検証".into(),
            locator: AssetLocator::Absolute {
                path: format!("/unused/{index}.wav").into(),
            },
            content_hash: Some("a".repeat(64)),
            probe: AssetProbe::Audio(AudioProbe {
                encoding: None,
                sha256: "a".repeat(64),
                duration_us: TimeUs(ms * 1000),
                sample_rate: 24000,
                channels: 1,
                frames: ms * 24,
                byte_length: ms * 48 + 44,
            }),
        },
        SpeechAudio {
            irodori_source: None,
            qwen_source: None,
            sbv2_source: None,
            asset_id,
            input_fingerprint: "b".repeat(64),
            engine_version: "1.0".into(),
            lip_sync: None,
        },
    )
}
