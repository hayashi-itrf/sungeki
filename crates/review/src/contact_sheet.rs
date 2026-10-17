//! Contact sheets are derived documents rendered with the same verified image/font pipeline.
use crate::ReviewedFrame;
use serde::Serialize;
use std::{fs, os::unix::fs::DirBuilderExt, path::Path, sync::atomic::AtomicBool};
use sungeki_project::*;
use sungeki_render::{FrameArtifact, FramePlan, RenderRuntime, render_frame_staged};

pub(crate) const PAGE_SIZE: usize = 12;
#[derive(Debug, Serialize)]
pub struct ContactSheet {
    pub page: usize,
    pub frame_indices: Vec<u64>,
    pub output: Option<FrameArtifact>,
    pub error: Option<String>,
}
pub(crate) fn render_page(
    frames: &[ReviewedFrame],
    page: usize,
    pages: usize,
    runtime: &RenderRuntime,
    directory: &Path,
    cancelled: &AtomicBool,
) -> Result<FrameArtifact, String> {
    let started = std::time::Instant::now();
    let project = document_checked(frames, page, pages, || {
        cancelled.load(std::sync::atomic::Ordering::SeqCst) || started.elapsed() >= runtime.deadline
    })?;
    let remaining = runtime
        .deadline
        .checked_sub(started.elapsed())
        .filter(|v| !v.is_zero())
        .ok_or("画像一覧の実行期限を超えました")?;
    let runtime = RenderRuntime {
        deadline: remaining,
        ..runtime.clone()
    };
    let plan = FramePlan::new(&project, 0).map_err(|e| e.to_string())?;
    let work = directory.join(format!("contact-sheet-{page:03}"));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&work)
        .map_err(|e| e.to_string())?;
    render_frame_staged(&plan, &runtime, &work, cancelled).map_err(|e| e.to_string())
}
fn id(n: usize) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn transform(x: f64, y: f64, width: f64, height: f64) -> Transform {
    Transform {
        x,
        y,
        width,
        height,
        opacity: 1.0,
        motion: None,
        fade: None,
        flip_x: None,
    }
}
struct Sheet {
    project: Project,
    next: usize,
}
impl Sheet {
    fn id(&mut self) -> String {
        self.next += 1;
        id(self.next)
    }
    fn clip(&mut self, content: ClipContent, transform: Transform) {
        let id = self.id();
        self.project.clips.push(Clip {
            id,
            track_id: id_track(),
            scene_id: None,
            start_us: TimeUs(0),
            duration_us: TimeUs(1_000_000),
            content,
            transform,
        });
    }
    fn text(&mut self, text: String, size: f64, color: &str, bounds: Transform) {
        self.clip(
            ClipContent::Text {
                outline: None,
                layout: None,
                text,
                font_size: size,
                color: color.into(),
            },
            bounds,
        );
    }
}
fn id_track() -> String {
    id(2)
}
fn timestamp(time: TimeUs) -> String {
    let ms = time.0 / 1000;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}
#[cfg(test)]
fn document(frames: &[ReviewedFrame], page: usize, pages: usize) -> Result<Project, String> {
    document_checked(frames, page, pages, || false)
}
fn document_checked(
    frames: &[ReviewedFrame],
    page: usize,
    pages: usize,
    mut stopped: impl FnMut() -> bool,
) -> Result<Project, String> {
    if frames.is_empty() || frames.len() > PAGE_SIZE || page == 0 || page > pages {
        return Err("画像一覧のページ指定が不正です".into());
    }
    let mut project = Project::empty(id(1), "代表画像一覧".into());
    project.canvas.width = 1216;
    project.canvas.height = (116 + frames.len().div_ceil(3) * 300) as u32;
    project.canvas.background = "#e8edf2".into();
    project.tracks.push(Track {
        id: id_track(),
        name: "画像一覧".into(),
        visible: true,
        muted: false,
        kind: TrackKind::Visual,
    });
    let mut sheet = Sheet { project, next: 2 };
    sheet.text(
        format!("代表画像  {page} / {pages}"),
        32.0,
        "#183849",
        transform(16.0, 12.0, 1184.0, 48.0),
    );
    sheet.text(
        "内容は未確認です。抜き出した画像だけで全編の合格とは判断できません。".into(),
        18.0,
        "#354858",
        transform(16.0, 64.0, 1184.0, 30.0),
    );
    for (index, frame) in frames.iter().enumerate() {
        if stopped() {
            return Err("画像一覧の作成が取り消されたか期限を超えました".into());
        }
        if frame.output.is_some() == frame.error.is_some() {
            return Err("画像の成功・失敗状態が不一致です".into());
        }
        let x = 16.0 + (index % 3) as f64 * 400.0;
        let y = 100.0 + (index / 3) as f64 * 300.0;
        sheet.clip(
            ClipContent::Shape {
                shape: ShapeKind::Rectangle,
                fill: "#ffffff".into(),
            },
            transform(x, y, 384.0, 284.0),
        );
        if let Some(output) = &frame.output {
            if output.frame_index != frame.sample.frame_index
                || output.time_us != frame.sample.time_us
                || output.width == 0
                || output.height == 0
                || !output.path.is_absolute()
            {
                return Err("代表画像と記録した時刻・寸法が一致しません".into());
            }
            let probe =
                sungeki_assets::probe_image(&output.path, sungeki_assets::ProbeLimits::default())
                    .map_err(|e| format!("代表画像を読み込めません: {e:?}"))?;
            if probe.sha256 != output.sha256
                || probe.byte_length != output.byte_length
                || probe.width != output.width
                || probe.height != output.height
                || probe.format != ImageFormat::Png
                || probe.orientation != 1
            {
                return Err("代表画像が生成時から変更されています".into());
            }
            let asset_id = sheet.id();
            sheet.project.assets.push(Asset {
                id: asset_id.clone(),
                locator: AssetLocator::Absolute {
                    path: output.path.to_str().ok_or("画像のパスが不正です")?.into(),
                },
                name: format!("frame-{}", output.frame_index),
                source: None,
                credit: String::new(),
                content_hash: Some(output.sha256.clone()),
                probe: AssetProbe::Image(probe),
            });
            let scale = (384.0 / output.width as f64).min(216.0 / output.height as f64);
            let width = output.width as f64 * scale;
            let height = output.height as f64 * scale;
            sheet.clip(
                ClipContent::Image { asset_id },
                transform(
                    x + (384.0 - width) / 2.0,
                    y + (216.0 - height) / 2.0,
                    width,
                    height,
                ),
            );
        } else {
            sheet.clip(
                ClipContent::Shape {
                    shape: ShapeKind::Rectangle,
                    fill: "#fbe7e7".into(),
                },
                transform(x, y, 384.0, 216.0),
            );
            sheet.text(
                "画像を生成できませんでした".into(),
                22.0,
                "#852b2b",
                transform(x + 16.0, y + 76.0, 352.0, 72.0),
            );
            sheet.text(
                "詳細は確認レポートを参照".into(),
                17.0,
                "#852b2b",
                transform(x + 16.0, y + 160.0, 352.0, 36.0),
            );
        }
        sheet.text(
            timestamp(frame.sample.time_us),
            20.0,
            "#183849",
            transform(x + 10.0, y + 222.0, 364.0, 30.0),
        );
        sheet.text(
            format!(
                "frame {}  /  {}",
                frame.sample.frame_index,
                if frame.output.is_some() {
                    "内容未確認"
                } else {
                    "生成失敗"
                }
            ),
            16.0,
            "#354858",
            transform(x + 10.0, y + 252.0, 364.0, 28.0),
        );
    }
    Ok(sheet.project)
}
#[cfg(test)]
#[path = "contact_sheet_tests.rs"]
mod tests;
