//! Portable collection metadata. Credits are preserved, never inferred licenses.
use super::Error;
use serde::{Deserialize, Serialize};
use sungeki_assets::CollectionPlan;
use sungeki_project::{Project, Voice};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    pub(super) format: String,
    pub(super) version: u32,
    pub(super) source_project_id: String,
    pub(super) source_project_sha256: String,
    pub(super) collected_project_id: String,
    pub(super) project_path: String,
    pub project_sha256: String,
    pub(super) checkpoints_included: bool,
    pub(super) checkpoints: Vec<super::history::ManifestEntry>,
    pub(super) assets: Vec<Asset>,
    characters: Vec<Character>,
    excluded_dependencies: Vec<String>,
    font_identity: Option<String>,
    limitations: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Asset {
    pub(super) asset_id: String,
    pub(super) name: String,
    pub(super) path: String,
    pub(super) sha256: String,
    pub(super) byte_length: u64,
    pub(super) credit: String,
    pub(super) license_status: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Character {
    character_id: String,
    pub(super) name: String,
    pub(super) credit: String,
    voice: Voice,
}
impl Manifest {
    pub fn new(
        source: &Project,
        collected: &Project,
        plan: &CollectionPlan,
    ) -> Result<Self, Error> {
        let mut assets = Vec::new();
        for asset in &source.assets {
            let path = plan
                .destination_for(0, &asset.id)
                .ok_or_else(|| Error::Invalid("manifestの素材参照がありません".into()))?;
            let file = plan
                .files
                .iter()
                .find(|f| f.relative_path == path)
                .ok_or_else(|| Error::Invalid("manifestの素材情報がありません".into()))?;
            assets.push(Asset {
                asset_id: asset.id.clone(),
                name: asset.name.clone(),
                path: path.into(),
                sha256: file.sha256.clone(),
                byte_length: file.byte_length,
                credit: asset.credit.clone(),
                license_status: "not_verified".into(),
            });
        }
        Ok(Self {
            format: "vvm-collection".into(), version: 1,
            source_project_id: source.id.clone(),
            source_project_sha256: sungeki_storage::project_hash(source)?,
            collected_project_id: collected.id.clone(), project_path: "project.vvm.json".into(),
            project_sha256: "0".repeat(64), checkpoints_included: false, checkpoints: Vec::new(), assets,
            characters: source.characters.iter().map(|c| Character {
                character_id: c.id.clone(), name: c.name.clone(), credit: c.credit.clone(),
                voice: c.voice.clone(),
            }).collect(),
            excluded_dependencies: ["tts_engines", "model_weights", "python", "node",
                "browser", "ffmpeg", "application_workers", "installed_fonts", "caches"]
                .into_iter().map(str::to_owned).collect(),
            font_identity: None,
            limitations: vec![
                "素材の利用条件・再配布可否は未確認です。クレジットは許諾を意味しません。".into(),
                "再出力にはSungeki Makerの描画環境とフォントが別途必要です。使用フォントの同一性は未確認です。".into(),
                "再合成には記載された音声エンジン・モデルが別途必要です。".into(),
                "名前付き保存時点・Undo履歴は含みません。".into(),
            ],
        })
    }
    pub fn characters_match(&self, project: &Project) -> bool {
        self.characters
            == project
                .characters
                .iter()
                .map(|c| Character {
                    character_id: c.id.clone(),
                    name: c.name.clone(),
                    credit: c.credit.clone(),
                    voice: c.voice.clone(),
                })
                .collect::<Vec<_>>()
    }
    pub fn add_history(&mut self, entries: Vec<super::history::ManifestEntry>) {
        self.checkpoints_included = !entries.is_empty();
        if self.checkpoints_included {
            self.limitations.pop();
            self.limitations.push("名前付き保存時点は同梱されています。取り込みには明示操作が必要です。Undo履歴は含みません。".into());
        }
        self.checkpoints = entries;
    }
    pub fn bytes(&self) -> Result<Vec<u8>, Error> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| Error::Invalid(e.to_string()))?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(Error::Limit("manifestの容量上限です".into()));
        }
        Ok(bytes)
    }
}
