//! Template placement uses the artwork's facing direction, never its name.
//! Subsequent inspector edits remain ordinary explicit transform changes.
use serde::Deserialize;
use std::sync::OnceLock;
use sungeki_project::{Clip, ClipContent};

#[derive(Deserialize)]
struct Preset {
    source_sha256: String,
    native_facing: Facing,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Facing {
    Left,
    Right,
}
fn presets() -> &'static [Preset; 2] {
    static PRESETS: OnceLock<[Preset; 2]> = OnceLock::new();
    PRESETS.get_or_init(|| {
        [
            include_str!("../../../presets/characters/zundamon-sakamoto-2.3-mouth.json"),
            include_str!("../../../presets/characters/metan-sakamoto-2.1-mouth.json"),
        ]
        .map(|json| serde_json::from_str(json).expect("bundled standing facing metadata"))
    })
}

pub(super) fn inward(clip: &Clip, slot: usize) -> Option<bool> {
    if let ClipContent::Psd { pose, .. } = &clip.content
        && matches!(slot, 1 | 2)
        && let Some(preset) = presets()
            .iter()
            .find(|p| p.source_sha256 == pose.source_sha256)
    {
        // Left slot wants right-facing art; right slot wants left-facing art.
        return Some(matches!(preset.native_facing, Facing::Left) == (slot == 1));
    }
    // Unknown/custom art cannot have its gaze inferred from a filename.
    clip.transform.flip_x
}

#[cfg(test)]
mod tests {
    use super::*;
    use sungeki_project::Project;

    #[test]
    fn both_characters_face_inward_after_swapping_sides_and_reapplying() {
        let mut p: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        for preset in presets() {
            for previous in [None, Some(false), Some(true)] {
                p.clips[0].transform.flip_x = previous;
                let ClipContent::Psd { pose, .. } = &mut p.clips[0].content else {
                    panic!()
                };
                pose.source_sha256 = preset.source_sha256.clone();
                for (slot, expected) in [(1, true), (2, false)] {
                    let result = crate::dialogue_geometry::fit(&p, &p.clips[0], slot).unwrap();
                    assert_eq!(result.flip_x, Some(expected));
                    assert_eq!(result.x < 960., slot == 1);
                }
                assert_eq!(inward(&p.clips[0], 0), previous);
            }
        }
    }

    #[test]
    fn custom_art_retains_explicit_orientation() {
        let mut p: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        for previous in [None, Some(false), Some(true)] {
            p.clips[0].transform.flip_x = previous;
            assert_eq!(inward(&p.clips[0], 1), previous);
            assert_eq!(inward(&p.clips[0], 2), previous);
        }
    }
}
