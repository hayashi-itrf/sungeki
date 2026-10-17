use sungeki_project::{IrodoriAudioSource, IrodoriIdentity, IrodoriSettings};
pub fn source() -> IrodoriAudioSource {
    IrodoriAudioSource {
        settings: IrodoriSettings {
            voice_id: "registered".into(),
            caption: None,
            cfg_scale: 3.,
            num_steps: 40,
            seed: 42,
        },
        identity: IrodoriIdentity {
            checkpoint: "model".into(),
            model_manifest_sha256: "a".repeat(64),
            codec_manifest_sha256: "b".repeat(64),
            engine_manifest_sha256: "c".repeat(64),
            voice_manifest_sha256: "d".repeat(64),
            sample_rate: 24000,
            channels: 1,
        },
        metadata_sha256: "e".repeat(64),
    }
}
