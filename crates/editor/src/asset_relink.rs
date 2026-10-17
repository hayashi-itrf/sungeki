use sungeki_project::*;

pub fn relink(
    project: &mut Project,
    asset_id: &str,
    locator: &AssetLocator,
    probe: &AssetProbe,
) -> Result<(), ValidationError> {
    let asset = project
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| ValidationError::new("対象の素材がありません".into()))?;
    if std::mem::discriminant(&asset.probe) != std::mem::discriminant(probe) {
        return Err(ValidationError::new(
            "同じ種類の素材を指定してください".into(),
        ));
    }
    if let (AssetProbe::Vrm(old), AssetProbe::Vrm(new)) = (&asset.probe, probe) {
        if old.inspection.kind != new.inspection.kind {
            return Err(ValidationError::new(
                "VRMモデルとVRMAモーションは入れ替えられません".into(),
            ));
        }
    }
    // Synthesized audio carries pronunciation provenance and time-based mouth data.
    // Moving its identical original is valid; replacing it requires synthesis again.
    let synthesized = project.clips.iter().any(|clip| {
        matches!(&clip.content,
        ClipContent::Speech { audio: Some(audio), .. } if audio.asset_id == asset_id)
    });
    if synthesized && &asset.probe != probe {
        return Err(ValidationError::new(
            "発話の生成音声は同じ原本だけ再指定できます。内容を変更する場合は再合成してください"
                .into(),
        ));
    }
    let asset = project
        .assets
        .iter_mut()
        .find(|asset| asset.id == asset_id)
        .unwrap();
    asset.locator = locator.clone();
    asset.content_hash = Some(probe.sha256().into());
    asset.probe = probe.clone();
    // apply_batch validates all referencing clips (duration, poses, expressions)
    // before publishing the candidate, so an incompatible replacement is atomic.
    Ok(())
}
