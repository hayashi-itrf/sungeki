use super::PreparedLocalInput;
use crate::voicevox::http::{check_cancel, config, connection, failure, read};
use crate::voicevox::{Stage, TransportError};
use reqwest::{Client, Url};
use serde_json::{Value, json};
use std::{sync::atomic::AtomicBool, time::Duration};
use sungeki_project::{LocalSpeechEngine, LocalVoiceSettings};
pub struct LocalSpeechClient {
    client: Client,
    base: Url,
    engine: LocalSpeechEngine,
}
impl LocalSpeechClient {
    pub fn new(
        base: &str,
        engine: LocalSpeechEngine,
        timeout: Duration,
    ) -> Result<Self, TransportError> {
        let (client, base) = connection(base, timeout)?;
        Ok(Self {
            client,
            base,
            engine,
        })
    }
    pub(super) async fn get(
        &self,
        path: &str,
        cancel: &AtomicBool,
    ) -> Result<Value, TransportError> {
        check_cancel(cancel)?;
        let mut url = self.base.clone();
        url.set_path(path);
        let r = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| failure(Stage::Catalog, e))?;
        let b = read(r, Stage::Catalog, 8 * 1024 * 1024).await?;
        check_cancel(cancel)?;
        serde_json::from_slice(&b).map_err(|_| config("音声エンジンの応答形式が不正です"))
    }
    pub async fn policy(
        &self,
        speaker: &str,
        cancel: &AtomicBool,
    ) -> Result<String, TransportError> {
        if speaker.len() != 36
            || speaker.bytes().enumerate().any(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b != b'-'
                } else {
                    !b.is_ascii_hexdigit()
                }
            })
        {
            return Err(config("話者UUIDが不正です"));
        }
        check_cancel(cancel)?;
        let mut url = self.base.clone();
        match self.engine {
            LocalSpeechEngine::AivisSpeech => {
                url.set_path("speaker_info");
                url.query_pairs_mut()
                    .append_pair("speaker_uuid", speaker)
                    .append_pair("resource_format", "url");
            }
            LocalSpeechEngine::Coeiroink => {
                url.set_path("v1/speaker_policy");
                url.query_pairs_mut().append_pair("speakerUuid", speaker);
            }
        }
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| failure(Stage::Catalog, e))?;
        // AivisSpeech 1.2 still embeds style samples even with resource_format=url.
        // Bound the body, but deserialize only policy rather than retaining images/audio.
        let bytes = read(response, Stage::Catalog, 32 * 1024 * 1024).await?;
        #[derive(serde::Deserialize)]
        struct Policy {
            policy: String,
        }
        let policy: Policy =
            serde_json::from_slice(&bytes).map_err(|_| config("利用条件の応答が不正です"))?;
        let policy = policy.policy;
        if policy.is_empty() || policy.len() > 256 * 1024 {
            return Err(config("話者の利用条件が不正です"));
        }
        check_cancel(cancel)?;
        Ok(policy)
    }
    pub fn engine(&self) -> LocalSpeechEngine {
        self.engine
    }
    pub async fn synthesize(
        &self,
        input: &PreparedLocalInput,
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>, TransportError> {
        check_cancel(cancel)?;
        if input.settings().engine() != self.engine {
            return Err(config("音声エンジンが一致しません"));
        }
        let (path, query, body) = match input.settings() {
            LocalVoiceSettings::AivisSpeech {
                style_id,
                speed_scale,
                pitch_scale,
                emotion_scale,
                tempo_dynamics_scale,
                volume_scale,
                pre_phoneme_length,
                post_phoneme_length,
                ..
            } => {
                let mut url = self.base.clone();
                url.set_path("audio_query");
                url.query_pairs_mut()
                    .append_pair("speaker", &style_id.to_string())
                    .append_pair("text", input.text());
                let r = self
                    .client
                    .post(url)
                    .send()
                    .await
                    .map_err(|e| failure(Stage::Analysis, e))?;
                let bytes = read(r, Stage::Analysis, 4 * 1024 * 1024).await?;
                let mut body: Value = serde_json::from_slice(&bytes)
                    .map_err(|_| config("AivisSpeechの合成クエリが不正です"))?;
                if !body.is_object() || !body.get("accent_phrases").is_some_and(Value::is_array) {
                    return Err(config("AivisSpeechの合成クエリが不正です"));
                }
                for (key, value) in [
                    ("speedScale", *speed_scale),
                    ("pitchScale", *pitch_scale),
                    ("intonationScale", *emotion_scale),
                    ("tempoDynamicsScale", *tempo_dynamics_scale),
                    ("volumeScale", *volume_scale),
                    ("prePhonemeLength", *pre_phoneme_length),
                    ("postPhonemeLength", *post_phoneme_length),
                ] {
                    body[key] = json!(value);
                }
                body["outputSamplingRate"] = json!(44100);
                body["outputStereo"] = json!(false);
                body["kana"] = json!(input.text());
                ("synthesis", Some(style_id.to_string()), body)
            }
            LocalVoiceSettings::Coeiroink {
                speaker_uuid,
                style_id,
                speed_scale,
                pitch_scale,
                intonation_scale,
                volume_scale,
                pre_phoneme_length,
                post_phoneme_length,
            } => (
                "v1/synthesis",
                None,
                json!({"speakerUuid":speaker_uuid,"styleId":style_id,"text":input.text(),"speedScale":speed_scale,"pitchScale":pitch_scale,"intonationScale":intonation_scale,"volumeScale":volume_scale,"prePhonemeLength":pre_phoneme_length,"postPhonemeLength":post_phoneme_length,"outputSamplingRate":44100,"sampledIntervalValue":0,"adjustedF0":[],"processingAlgorithm":"RESAMPLING","prosodyDetail":[]}),
            ),
        };
        check_cancel(cancel)?;
        let mut url = self.base.clone();
        url.set_path(path);
        if let Some(style) = query {
            url.query_pairs_mut().append_pair("speaker", &style);
        }
        let r = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_vec(&body).map_err(|_| config("合成要求の符号化に失敗しました"))?)
            .send()
            .await
            .map_err(|e| failure(Stage::Synthesis, e))?;
        let bytes = read(r, Stage::Synthesis, sungeki_media::MAX_WAV_BYTES).await?;
        check_cancel(cancel)?;
        Ok(bytes)
    }
}
