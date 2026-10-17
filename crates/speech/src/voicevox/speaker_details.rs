use super::{
    EngineInfo, Speaker, TransportError, VoicevoxDiscovery,
    catalog::{identifier, invalid},
};
use serde::Deserialize;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakerDetails {
    pub engine: EngineInfo,
    pub core_version: String,
    pub speaker: Speaker,
    pub engine_terms: String,
    pub speaker_policy: String,
    pub default_sampling_rate: u32,
}
#[derive(Deserialize)]
struct Terms {
    uuid: String,
    name: String,
    terms_of_service: String,
    default_sampling_rate: u32,
}
#[derive(Deserialize)]
struct Info {
    policy: String,
}
impl VoicevoxDiscovery {
    /// Resources in speaker_info are ignored: do not fetch arbitrary image/audio URLs.
    /// Rechecks the chosen speaker's metadata; does not attest process identity.
    pub async fn speaker_details(
        &self,
        expected: &EngineInfo,
        core: &str,
        speaker_id: &str,
        cancelled: &AtomicBool,
    ) -> Result<SpeakerDetails, TransportError> {
        identifier(speaker_id)?;
        let before = self.catalog(expected, core, cancelled).await?;
        let speaker = before
            .speakers
            .into_iter()
            .find(|s| s.speaker_uuid == speaker_id)
            .ok_or_else(|| invalid("選択した話者は利用できません。再取得してください"))?;
        let terms: Terms = self.get("engine_manifest", None, cancelled).await?;
        if terms.uuid != expected.engine_id
            || terms.name != expected.name
            || terms.default_sampling_rate == 0
        {
            return Err(invalid("エンジン情報または出力形式が不正です"));
        }
        let info: Info = self
            .get_pairs(
                "speaker_info",
                &[
                    ("core_version", core),
                    ("speaker_uuid", speaker_id),
                    ("resource_format", "url"),
                ],
                cancelled,
            )
            .await?;
        policy(&terms.terms_of_service)?;
        policy(&info.policy)?;
        let after = self.catalog(expected, core, cancelled).await?;
        if after.speakers.iter().find(|s| s.speaker_uuid == speaker_id) != Some(&speaker) {
            return Err(invalid(
                "取得中に話者情報が変わりました。再取得してください",
            ));
        }
        Ok(SpeakerDetails {
            engine: after.engine,
            core_version: core.into(),
            speaker,
            engine_terms: terms.terms_of_service,
            speaker_policy: info.policy,
            default_sampling_rate: terms.default_sampling_rate,
        })
    }
}
fn policy(text: &str) -> Result<(), TransportError> {
    if text.len() > 256 * 1024 || text.contains('\0') {
        return Err(invalid("利用条件の文章が不正、または256KiBを超えています"));
    }
    // Empty policy is shown explicitly as not supplied, never as permission.
    Ok(())
}
