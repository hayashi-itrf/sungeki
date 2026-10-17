use super::*;

#[test]
fn automatic_layout_faces_both_registered_artworks_inward_and_allows_later_edits() {
    let psd: Project = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let source = psd
        .assets
        .iter()
        .find(|a| matches!(a.probe, AssetProbe::Psd(_)))
        .unwrap();
    let ClipContent::Psd {
        pose,
        speech: Some(binding),
        ..
    } = &psd
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Psd { .. }))
        .unwrap()
        .content
    else {
        panic!()
    };
    let presets: Vec<serde_json::Value> = [
        include_str!("../../../../presets/characters/zundamon-sakamoto-2.3-mouth.json"),
        include_str!("../../../../presets/characters/metan-sakamoto-2.1-mouth.json"),
    ]
    .iter()
    .map(|s| serde_json::from_str(s).unwrap())
    .collect();
    for swapped in [false, true] {
        let (p, mut recipe, speech) = fixture();
        for index in 0..2 {
            let preset = &presets[if swapped { 1 - index } else { index }];
            let hash = preset["source_sha256"].as_str().unwrap().to_owned();
            let mut asset = source.clone();
            asset.id = id(900 + index as u32);
            asset.content_hash = Some(hash.clone());
            let AssetProbe::Psd(info) = &mut asset.probe else {
                panic!()
            };
            info.sha256 = hash.clone();
            let mut pose = pose.clone();
            pose.source_sha256 = hash;
            let mut binding = binding.clone();
            binding.character_id = recipe.roles[index].character.id.clone();
            recipe.roles[index].standing = BuildStanding::Psd {
                asset_id: asset.id.clone(),
                pose,
                speech: binding,
                // Deliberately start with both sides facing outward.
                flip_x: index == 1,
            };
            recipe.assets.push(asset);
        }
        let stage = synthesize(&p, &recipe, &speech);
        let mut ids = layout_ids();
        ids.bgm = None;
        let full = apply_batch(
            &stage,
            &plan_build_layout(&stage, &recipe, &speech, &ids).unwrap(),
        )
        .unwrap();
        for scene in &ids.scenes {
            for pair in &scene.standing {
                for (index, id) in pair.iter().enumerate() {
                    let clip = full.clips.iter().find(|c| &c.id == id).unwrap();
                    assert_eq!(clip.transform.flip_x, Some(index == 0));
                }
            }
        }
        let selected = full
            .clips
            .iter()
            .find(|c| c.id == ids.scenes[0].standing[0][1])
            .unwrap();
        let mut transform = selected.transform.clone();
        transform.flip_x = Some(true);
        let edited = apply_batch(
            &full,
            &[Command::SetTransform {
                clip_id: selected.id.clone(),
                transform,
            }],
        )
        .unwrap();
        let saved: Project = serde_json::from_slice(&serde_json::to_vec(&edited).unwrap()).unwrap();
        assert_eq!(saved, edited);
        assert_eq!(
            saved
                .clips
                .iter()
                .find(|c| c.id == selected.id)
                .unwrap()
                .transform
                .flip_x,
            Some(true)
        );
    }
}
