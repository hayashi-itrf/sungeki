use rmcp::{ErrorData, model::Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use sungeki_protocol::{PresetRequest, SessionRequest};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    after_id: Option<String>,
    limit: u32,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct GetArgs {
    id: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SaveArgs {
    id: String,
    expected_revision: u32,
    preset: sungeki_protocol::Preset,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DeleteArgs {
    id: String,
    expected_revision: u32,
}

pub fn definitions() -> Vec<Tool> {
    vec![
        crate::tools::tool::<sungeki_protocol::InstantiatePresetArgs>(
            "instantiate_preset",
            "保存したキャラクタープリセットを指定sessionへ新規追加。preset_revisionと作品expected_revision、新規人物/素材/立ち絵ID対応を固定して送る。一回でUndo可能。同一operation_idは再送可。声のみ・テンプレの適用は未対応。",
        ),
        crate::tools::tool::<ListArgs>(
            "list_presets",
            "ユーザー保存プリセットの名前・種類一覧。limitは1〜20。next_after_idを次のafter_idへ渡す。同梱の固定制作プリセットとは別。",
        ),
        crate::tools::tool::<GetArgs>(
            "get_preset",
            "保存プリセットの全設定とrevisionを取得。削除済みはpreset=null。素材の存在・利用可能性は別途確認が必要。",
        ),
        crate::tools::tool::<SaveArgs>(
            "save_preset",
            "声・キャラ・制作テンプレを作品外へ保存。新規は新しいUUIDとexpected_revision=0、更新は取得したrevision。作品相対素材は不可。既存作品を変更しない。",
        ),
        crate::tools::tool::<DeleteArgs>(
            "delete_preset",
            "取得したrevisionを指定して保存プリセットを削除。既存作品や素材原本を削除しない。削除済みIDは再利用不可。",
        ),
    ]
}
pub fn request(name: &str, args: Value) -> Result<Option<SessionRequest>, ErrorData> {
    use crate::dispatch::parse;
    let request = match name {
        "instantiate_preset" => {
            return Ok(Some(SessionRequest::InstantiatePreset {
                args: parse(args)?,
            }));
        }
        "list_presets" => {
            let a: ListArgs = parse(args)?;
            PresetRequest::List {
                after_id: a.after_id,
                limit: a.limit,
            }
        }
        "get_preset" => {
            let a: GetArgs = parse(args)?;
            PresetRequest::Get { id: a.id }
        }
        "save_preset" => {
            let a: SaveArgs = parse(args)?;
            PresetRequest::Save {
                id: a.id,
                expected_revision: a.expected_revision,
                preset: a.preset,
            }
        }
        "delete_preset" => {
            let a: DeleteArgs = parse(args)?;
            PresetRequest::Delete {
                id: a.id,
                expected_revision: a.expected_revision,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(SessionRequest::Presets { request }))
}
