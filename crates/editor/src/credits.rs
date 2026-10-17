use std::collections::BTreeSet;
use sungeki_project::*;

fn error(message: &str) -> ValidationError {
    ValidationError::new(message.into())
}
fn add_lines(credits: &mut BTreeSet<String>, value: &str) {
    credits.extend(
        value
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned),
    );
}

/// Collect registered attribution, not a license check. Generated speech assets use character credit.
pub(crate) fn text_for_clips(p: &Project, ids: &[String]) -> Result<String, ValidationError> {
    let mut credits = BTreeSet::new();
    for id in ids {
        let clip = p
            .clips
            .iter()
            .find(|c| c.id == *id)
            .ok_or_else(|| error("クレジットの対象クリップが見つかりません"))?;
        let extra_assets: Vec<&str> = match &clip.content {
            ClipContent::Sprite { sprites, .. } => sprites.asset_ids(),
            ClipContent::Vrm { standing } => vec![
                standing.model_asset_id.as_str(),
                standing.motion_asset_id.as_str(),
            ],
            _ => vec![],
        };
        {
            for id in extra_assets {
                let asset = p
                    .assets
                    .iter()
                    .find(|a| a.id == id)
                    .ok_or_else(|| error("クレジットの対象素材が見つかりません"))?;
                if asset.credit.trim().is_empty() {
                    return Err(error(&format!(
                        "素材「{}」のクレジットを入力してください",
                        asset.name
                    )));
                }
                add_lines(&mut credits, &asset.credit);
            }
        }
        let (asset_id, character_id) = match &clip.content {
            ClipContent::Image { asset_id }
            | ClipContent::Audio { asset_id, .. }
            | ClipContent::Video { asset_id, .. } => (Some(asset_id), None),
            ClipContent::Psd {
                asset_id, speech, ..
            } => (Some(asset_id), speech.as_ref().map(|s| &s.character_id)),
            ClipContent::Vrm { standing } => (None, standing.character_id.as_ref()),
            ClipContent::Sprite { character_id, .. } => (None, character_id.as_ref()),
            ClipContent::Speech { character_id, .. } => (None, Some(character_id)),
            _ => (None, None),
        };
        if let Some(id) = asset_id {
            let asset = p
                .assets
                .iter()
                .find(|a| a.id == *id)
                .ok_or_else(|| error("クレジットの対象素材が見つかりません"))?;
            if asset.credit.trim().is_empty() {
                return Err(error(&format!(
                    "素材「{}」のクレジットを入力してください",
                    asset.name
                )));
            }
            add_lines(&mut credits, &asset.credit);
        }
        if let Some(id) = character_id {
            let character = p
                .characters
                .iter()
                .find(|c| c.id == *id)
                .ok_or_else(|| error("クレジットの対象話者が見つかりません"))?;
            if character.credit.trim().is_empty() {
                return Err(error(&format!(
                    "話者「{}」のクレジットを入力してください",
                    character.name
                )));
            }
            add_lines(&mut credits, &character.credit);
        }
    }
    if credits.is_empty() {
        return Err(error(
            "このシーンにはクレジットを持つ素材・話者がありません",
        ));
    }
    Ok(credits.into_iter().collect::<Vec<_>>().join("\n"))
}

pub(crate) fn refresh(
    p: &mut Project,
    clip_id: &str,
    scene_id: &str,
) -> Result<(), ValidationError> {
    if !p.scenes.iter().any(|s| s.id == scene_id) {
        return Err(error("クレジットの対象シーンが見つかりません"));
    }
    let ids = p
        .clips
        .iter()
        .filter(|c| c.scene_id.as_deref() == Some(scene_id))
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    let text = text_for_clips(p, &ids)?;
    let clip = p
        .clips
        .iter_mut()
        .find(|c| c.id == clip_id)
        .ok_or_else(|| error("クレジットを表示する文字クリップが見つかりません"))?;
    if clip.scene_id.as_deref() != Some(scene_id) {
        return Err(error("同じシーンに所属する文字クリップを指定してください"));
    }
    let ClipContent::Text { text: current, .. } = &mut clip.content else {
        return Err(error("クレジットの表示先は文字クリップを指定してください"));
    };
    *current = text;
    Ok(())
}
