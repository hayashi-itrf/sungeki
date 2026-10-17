use std::collections::BTreeSet;
use sungeki_project::*;

/// Explicit ordered layout. Only the requested speech clips move; scene bounds stay fixed.
pub(crate) fn layout(
    p: &mut Project,
    ids: &[String],
    start: TimeUs,
    gap: TimeUs,
) -> Result<(), ValidationError> {
    let error = |text: &str| ValidationError::new(text.into());
    let selected: BTreeSet<_> = ids.iter().collect();
    if ids.is_empty() || ids.len() > 200 || selected.len() != ids.len() {
        return Err(error("並べるセリフを重複なしで1〜200件指定してください"));
    }
    if start.0 > MAX_TIME_US || gap.0 > MAX_TIME_US {
        return Err(error("開始位置と間隔は0〜7日で指定してください"));
    }
    let mut cursor = start.0;
    let mut placements = Vec::new();
    for id in ids {
        let clip = p
            .clips
            .iter()
            .find(|c| c.id == *id)
            .ok_or_else(|| error("並べるセリフが見つかりません"))?;
        let ClipContent::Speech {
            audio: Some(audio), ..
        } = &clip.content
        else {
            return Err(error("選択したセリフすべての音声を先に生成してください"));
        };
        let duration = p
            .assets
            .iter()
            .find_map(|a| match &a.probe {
                AssetProbe::Audio(probe) if a.id == audio.asset_id => Some(probe.duration_us.0),
                _ => None,
            })
            .ok_or_else(|| error("合成音声の長さを取得できません"))?;
        let end = cursor
            .checked_add(duration)
            .filter(|e| *e <= MAX_TIME_US)
            .ok_or_else(|| error("並べたセリフが7日の範囲を超えます"))?;
        if let Some(scene_id) = &clip.scene_id {
            let scene = p
                .scenes
                .iter()
                .find(|s| s.id == *scene_id)
                .ok_or_else(|| error("所属シーンが見つかりません"))?;
            if cursor < scene.start_us.0 || end > scene.start_us.0 + scene.duration_us.0 {
                return Err(error(
                    "並べたセリフが所属シーンの範囲を超えます。開始位置を調整するか、先にシーンの長さを変更してください",
                ));
            }
        }
        if p.clips.iter().any(|c| {
            !selected.contains(&c.id)
                && matches!(c.content, ClipContent::Speech { .. })
                && cursor < c.start_us.0 + c.duration_us.0
                && c.start_us.0 < end
        }) {
            return Err(error(
                "選択していないセリフと重なります。対象または開始位置を変更してください",
            ));
        }
        placements.push((id, cursor, duration));
        cursor = end
            .checked_add(gap.0)
            .ok_or_else(|| error("配置時刻が大きすぎます"))?;
    }
    for (id, start, duration) in placements {
        let clip = p.clips.iter_mut().find(|c| c.id == *id).unwrap();
        clip.start_us = TimeUs(start);
        clip.duration_us = TimeUs(duration);
    }
    Ok(())
}
