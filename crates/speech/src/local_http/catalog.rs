use super::LocalSpeechClient;
use crate::voicevox::{TransportError, http::config};
use serde::Deserialize;
use std::sync::atomic::AtomicBool;
use sungeki_project::LocalSpeechEngine;
#[derive(Debug, Clone, Deserialize)]
pub struct LocalSpeaker {
    pub speaker_uuid: String,
    pub name: String,
    pub styles: Vec<LocalStyle>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct LocalStyle {
    pub id: i32,
    pub name: String,
}
pub struct LocalCatalog {
    pub version: String,
    pub speakers: Vec<LocalSpeaker>,
}
impl LocalSpeechClient {
    pub async fn catalog(&self, cancel: &AtomicBool) -> Result<LocalCatalog, TransportError> {
        let (version, speakers) = match self.engine() {
            LocalSpeechEngine::AivisSpeech => {
                let manifest = self.get("engine_manifest", cancel).await?;
                if manifest.get("name").and_then(|v| v.as_str()) != Some("AivisSpeech Engine") {
                    return Err(config("接続先はAivisSpeech Engineではありません"));
                }
                (
                    self.get("version", cancel).await?,
                    self.get("speakers", cancel).await?,
                )
            }
            LocalSpeechEngine::Coeiroink => (
                self.get("v1/engine_info", cancel).await?,
                self.get("v1/speakers", cancel).await?,
            ),
        };
        let version = version
            .as_str()
            .or_else(|| version.get("version").and_then(|v| v.as_str()))
            .ok_or_else(|| config("エンジン版を確認できません"))?
            .to_owned();
        if version.is_empty() || version.len() > 128 || version.chars().any(char::is_control) {
            return Err(config("エンジン版が不正です"));
        }
        use sha2::{Digest, Sha256};
        let catalog_hash = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&speakers).map_err(|_| config("話者情報を固定できません"))?
            )
        );
        let version = format!("{}@{}", version, catalog_hash);
        if version.len() > 128 {
            return Err(config("エンジン版が長すぎます"));
        }
        let mut speakers = speakers;
        if self.engine() == LocalSpeechEngine::Coeiroink {
            for speaker in speakers
                .as_array_mut()
                .ok_or_else(|| config("話者一覧が不正です"))?
            {
                speaker["speaker_uuid"] = speaker["speakerUuid"].take();
                speaker["name"] = speaker["speakerName"].take();
                for style in speaker["styles"]
                    .as_array_mut()
                    .ok_or_else(|| config("スタイル一覧が不正です"))?
                {
                    style["id"] = style["styleId"].take();
                    style["name"] = style["styleName"].take();
                }
            }
        }
        let speakers: Vec<LocalSpeaker> =
            serde_json::from_value(speakers).map_err(|_| config("話者一覧が不正です"))?;
        if speakers.len() > 4096
            || speakers.iter().any(|s| {
                s.name.is_empty()
                    || s.name.len() > 1024
                    || s.speaker_uuid.len() != 36
                    || s.styles.len() > 4096
                    || s.styles
                        .iter()
                        .any(|s| s.name.is_empty() || s.name.len() > 1024)
            })
        {
            return Err(config("話者一覧が上限を超えています"));
        }
        Ok(LocalCatalog { version, speakers })
    }
    pub async fn verify(
        &self,
        input: &super::PreparedLocalInput,
        cancel: &AtomicBool,
    ) -> Result<(), TransportError> {
        let catalog = self.catalog(cancel).await?;
        let (uuid, id) = input.settings().selection();
        if catalog.version != input.version()
            || !catalog
                .speakers
                .iter()
                .any(|s| s.speaker_uuid == uuid && s.styles.iter().any(|s| s.id == id))
        {
            return Err(config(
                "エンジン版または話者が変更されています。話者を選び直してください",
            ));
        }
        Ok(())
    }
}
