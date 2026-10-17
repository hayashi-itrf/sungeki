use super::http::{check_cancel, config, connection, failure, read};
use super::{AudioQuery, Stage, TransportError, VoicevoxSettings, prepare_synthesis_query};
use reqwest::{Client, Url};
use std::{sync::atomic::AtomicBool, time::Duration};

const MAX_AUDIO_BYTES: usize = sungeki_media::MAX_WAV_BYTES;

pub struct VoicevoxClient {
    client: Client,
    base: Url,
    core_version: String,
}
/// HTTP success is not proof of valid/complete playable audio. The media layer
/// must probe these bytes before storing them as a synthesis artifact.
pub struct VoicevoxResponse {
    pub query: AudioQuery,
    pub unverified_wav: Vec<u8>,
}
impl VoicevoxClient {
    /// Only direct loopback HTTP is supported. DNS, proxy, redirect and implicit
    /// retries cannot route speech text to another service or duplicate a job.
    pub fn new(base: &str, core_version: &str, timeout: Duration) -> Result<Self, TransportError> {
        if core_version.trim().is_empty()
            || core_version.len() > 128
            || core_version.chars().any(char::is_control)
        {
            return Err(config(
                "利用するVOICEVOX coreのバージョンを指定してください",
            ));
        }
        let (client, base) = connection(base, timeout)?;
        Ok(Self {
            client,
            base,
            core_version: core_version.into(),
        })
    }

    /// Use one immutable input for both HTTP stages. Engine identity must also
    /// be verified by the lease owner before dispatch; URL equality is insufficient.
    pub async fn synthesize_prepared(
        &self,
        input: &crate::PreparedVoicevoxInput,
        cancelled: &AtomicBool,
    ) -> Result<VoicevoxResponse, TransportError> {
        if input.identity().core_version != self.core_version {
            return Err(config("固定入力と接続先のcoreバージョンが一致しません"));
        }
        self.synthesize(&input.resolved().text, input.settings(), cancelled)
            .await
    }

    /// `text` must already be resolved by resolve_text; no second dictionary pass.
    /// The owner holds its engine lease until this future completes. Cancellation
    /// drains the in-flight response (up to the deadline) and discards the result.
    /// Dropping this future does NOT cancel remote engine execution.
    pub async fn synthesize(
        &self,
        text: &str,
        settings: &VoicevoxSettings,
        cancelled: &AtomicBool,
    ) -> Result<VoicevoxResponse, TransportError> {
        check_cancel(cancelled)?;
        settings
            .validate()
            .map_err(|error| TransportError::Input(error.into()))?;
        if text.trim().is_empty()
            || text.contains('\0')
            || text.len() > crate::MAX_SPEECH_OUTPUT_BYTES
        {
            return Err(config(
                "解決済みの発音文字列が空・不正、または256KiBを超えています",
            ));
        }
        let mut url = self.endpoint("audio_query", settings.style_id);
        url.query_pairs_mut()
            .append_pair("text", text)
            .append_pair("enable_katakana_english", "true");
        let response = self
            .client
            .post(url)
            .send()
            .await
            .map_err(|e| failure(Stage::Analysis, e))?;
        let bytes = read(response, Stage::Analysis, super::query::MAX_QUERY_BYTES).await?;
        check_cancel(cancelled)?;
        let query = prepare_synthesis_query(&bytes, settings).map_err(TransportError::Input)?;
        let mut url = self.endpoint("synthesis", settings.style_id);
        url.query_pairs_mut().append_pair(
            "enable_interrogative_upspeak",
            if settings.enable_interrogative_upspeak {
                "true"
            } else {
                "false"
            },
        );
        let body =
            serde_json::to_vec(&query).map_err(|_| config("合成クエリをエンコードできません"))?;
        check_cancel(cancelled)?;
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| failure(Stage::Synthesis, e))?;
        let unverified_wav = read(response, Stage::Synthesis, MAX_AUDIO_BYTES).await?;
        check_cancel(cancelled)?;
        Ok(VoicevoxResponse {
            query,
            unverified_wav,
        })
    }
    fn endpoint(&self, path: &str, style_id: u32) -> Url {
        let mut url = self.base.clone();
        url.set_path(path);
        url.query_pairs_mut()
            .append_pair("speaker", &style_id.to_string())
            .append_pair("core_version", &self.core_version);
        url
    }
}
