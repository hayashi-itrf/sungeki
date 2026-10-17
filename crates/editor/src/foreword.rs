use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::*;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ForewordTemplate {
    pub preset_id: String,
    pub background_asset_id: String,
    pub text: String,
    pub duration_us: TimeUs,
    pub scene_id: String,
    pub track_id: String,
    // Background, blur, shade, text: new IDs, in back-to-front order.
    pub clip_ids: Vec<String>,
}
#[derive(Deserialize)]
struct Preset {
    preset_id: String,
    canvas: PresetCanvas,
    fade_us: u64,
    sigma: f64,
    shade_opacity: f64,
    font_size: f64,
    text_rect: Rect,
}
#[derive(Deserialize)]
struct PresetCanvas {
    width: u32,
    height: u32,
    fps: u32,
}
#[derive(Deserialize)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
fn error(text: &str) -> ValidationError {
    ValidationError::new(text.into())
}
pub(crate) fn prepend(p: &mut Project, t: &ForewordTemplate) -> Result<(), ValidationError> {
    let definition = match t.preset_id.as_str() {
        "foreword-wide-v1" => include_str!("../../../templates/foreword-wide-v1.json"),
        "foreword-portrait-v1" => include_str!("../../../templates/foreword-portrait-v1.json"),
        _ => return Err(error("未対応の前書きプリセットです")),
    };
    let preset: Preset =
        serde_json::from_str(definition).map_err(|_| error("前書きプリセットの定義が不正です"))?;
    if t.preset_id != preset.preset_id
        || t.clip_ids.len() != 4
        || p.canvas.width != preset.canvas.width
        || p.canvas.height != preset.canvas.height
        || p.canvas.fps.numerator != preset.canvas.fps
        || p.canvas.fps.denominator != 1
    {
        return Err(error(
            "前書きはプリセットに対応する画面サイズ/30fps、4つの新規クリップIDが必要です",
        ));
    }
    if t.text.trim().is_empty()
        || t.text.len() > 4096
        || t.text.contains('\0')
        || t.text.lines().count() > 4
    {
        return Err(error(
            "前書きの文章は空でない4行・4096バイト以内で指定してください",
        ));
    }
    if t.duration_us.0 < preset.fade_us * 2 || t.duration_us.0 > MAX_TIME_US {
        return Err(error(
            "前書きの長さはフェードの合計1.6秒以上、7日以内にしてください",
        ));
    }
    if !p.assets.iter().any(|a| {
        a.id == t.background_asset_id
            && matches!(a.probe, AssetProbe::Image(_))
            && !a.credit.trim().is_empty()
    }) {
        return Err(error("クレジット付きの登録済み背景画像を選んでください"));
    }
    let scenes = p.scenes.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    let clips = p
        .clips
        .iter()
        .filter(|c| c.scene_id.is_none())
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    crate::scene_edit::move_selection(p, &scenes, &[], &clips, t.duration_us.0 as i64)?;
    p.scenes.push(Scene {
        fade: None,
        id: t.scene_id.clone(),
        name: "前書き".into(),
        start_us: TimeUs(0),
        duration_us: t.duration_us,
    });
    p.tracks.push(Track {
        id: t.track_id.clone(),
        name: "前書き".into(),
        visible: true,
        muted: false,
        kind: TrackKind::Visual,
    });
    let full = Transform {
        motion: None,
        x: 0.,
        y: 0.,
        width: p.canvas.width as f64,
        height: p.canvas.height as f64,
        opacity: 1.,
        fade: None,
        flip_x: None,
    };
    let out = Some(VisualFade {
        in_us: TimeUs(0),
        out_us: TimeUs(preset.fade_us),
    });
    let text = Transform {
        motion: None,
        x: preset.text_rect.x,
        y: preset.text_rect.y,
        width: preset.text_rect.width,
        height: preset.text_rect.height,
        fade: Some(VisualFade {
            in_us: TimeUs(preset.fade_us),
            out_us: TimeUs(preset.fade_us),
        }),
        ..full.clone()
    };
    let layers = [
        (
            ClipContent::Image {
                asset_id: t.background_asset_id.clone(),
            },
            full.clone(),
        ),
        (
            ClipContent::Blur {
                sigma: preset.sigma,
            },
            Transform {
                motion: None,
                fade: out.clone(),
                ..full.clone()
            },
        ),
        (
            ClipContent::Shape {
                shape: ShapeKind::Rectangle,
                fill: "#000000".into(),
            },
            Transform {
                motion: None,
                opacity: preset.shade_opacity,
                fade: out,
                ..full
            },
        ),
        (
            ClipContent::Text {
                outline: None,
                layout: None,
                text: t.text.clone(),
                font_size: preset.font_size,
                color: "#ffffff".into(),
            },
            text,
        ),
    ];
    for ((content, transform), id) in layers.into_iter().zip(&t.clip_ids) {
        let mut clip = Clip {
            id: id.clone(),
            track_id: t.track_id.clone(),
            scene_id: Some(t.scene_id.clone()),
            start_us: TimeUs(0),
            duration_us: t.duration_us,
            content,
            transform,
        };
        if t.preset_id == "foreword-portrait-v1"
            && matches!(clip.content, ClipContent::Image { .. })
        {
            clip.transform = crate::build_portrait::fit(p, &clip, true)?;
        }
        p.clips.push(clip);
    }
    // Batch-level validation rejects ID collisions, shifted scene bounds and overflow.
    crate::track_separation::separate(p);
    Ok(())
}
