use sungeki_project::*;

/// Fit 2D artwork inside the requested box, bottom-aligned and aspect-correct.
/// VRM uses that box as its camera viewport. Known artwork faces the canvas center.
pub(super) fn fit(
    project: &Project,
    assets: &[Asset],
    clip: &mut Clip,
) -> Result<(), ValidationError> {
    let asset_id = match &clip.content {
        ClipContent::Image { asset_id } | ClipContent::Psd { asset_id, .. } => {
            Some(asset_id.as_str())
        }
        ClipContent::Sprite { sprites, pose, .. } => {
            Some(sprites.asset_id(pose.as_deref(), false)?)
        }
        _ => None,
    };
    if let Some(id) = asset_id {
        let asset = assets
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| ValidationError::new("立ち絵素材が見つかりません".into()))?;
        let (w, h) = match &asset.probe {
            AssetProbe::Image(p) if (5..=8).contains(&p.orientation) => (p.height, p.width),
            AssetProbe::Image(p) => (p.width, p.height),
            AssetProbe::Psd(p) => (p.width, p.height),
            _ => return Err(ValidationError::new("立ち絵の寸法を取得できません".into())),
        };
        let t = &mut clip.transform;
        let scale = (t.width / f64::from(w)).min(t.height / f64::from(h));
        let width = f64::from(w) * scale;
        let height = f64::from(h) * scale;
        t.x += (t.width - width) / 2.;
        t.y += t.height - height;
        t.width = width;
        t.height = height;
    }
    let center = clip.transform.x + clip.transform.width / 2.;
    let side = if center < f64::from(project.canvas.width) / 2. {
        1
    } else {
        2
    };
    clip.transform.flip_x = super::standing_facing::inward(clip, side);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fitting_preserves_aspect_and_known_characters_face_inward_on_both_sides() {
        let mut p: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        let preset: serde_json::Value = serde_json::from_str(include_str!(
            "../../../presets/characters/metan-sakamoto-2.1-mouth.json"
        ))
        .unwrap();
        p.canvas.width = 1920;
        p.canvas.height = 1080;
        for (x, flip) in [(0., true), (960., false)] {
            let mut clip = p.clips[0].clone();
            let ClipContent::Psd { pose, asset_id, .. } = &mut clip.content else {
                panic!()
            };
            pose.source_sha256 = preset["source_sha256"].as_str().unwrap().into();
            let AssetProbe::Psd(probe) =
                &p.assets.iter().find(|a| &a.id == asset_id).unwrap().probe
            else {
                panic!()
            };
            let ratio = f64::from(probe.width) / f64::from(probe.height);
            clip.transform.x = x;
            clip.transform.y = 100.;
            clip.transform.width = 960.;
            clip.transform.height = 700.;
            fit(&p, &p.assets, &mut clip).unwrap();
            let t = clip.transform;
            assert!((t.width / t.height - ratio).abs() < 1e-10);
            assert!(t.x >= x && t.x + t.width <= x + 960.);
            assert!((t.y + t.height - 800.).abs() < 1e-10);
            assert_eq!(t.flip_x, Some(flip));
        }
    }
}
