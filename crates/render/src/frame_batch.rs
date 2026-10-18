//! Bounded deferred raster calls. Only validated whole batches enter the cache.
use super::*;
use std::path::Path;

pub(super) struct PendingFrame {
    pub input: PathBuf,
    pub resources: PathBuf,
    pub index: usize,
    pub frame_index: u64,
    pub key: [u8; 32],
    pub layers: Vec<Layer>,
}
impl FrameCache {
    pub fn batched() -> Self {
        Self {
            deferred: true,
            ..Self::default()
        }
    }
    pub fn flush(&mut self, runner: &Runner<'_>, canvas: &Canvas) -> Result<(), RenderError> {
        crate::originals::cancelled(runner.flag)?;
        if self.pending.is_empty() {
            return Ok(());
        }
        if self.pending.len() > 32 {
            return Err(error("画像合成バッチが上限を超えました"));
        }
        let manifest = runner
            .directory
            .join(format!("batch-{}.json", self.pending[0].index));
        let frames: Vec<_> = self.pending.iter().map(|p| json!({
            "input": p.input, "resources": p.resources, "output": format!("frame-{}.png", p.index)
        })).collect();
        write_new(
            &manifest,
            &serde_json::to_vec(&json!({"version":1, "frames":frames}))?,
        )?;
        let mut results = Vec::new();
        for pending in &self.pending {
            crate::originals::cancelled(runner.flag)?;
            let name = format!("frame-{}.png", pending.index);
            runner.check_active()?;
            let report = sungeki_raster::files::render(
                &mut self.raster,
                &pending.input,
                &pending.resources,
                &runner.directory.join("font.ttc"),
                &runner.runtime.visual()?.font_sha256,
                &runner.directory.join(&name),
                runner.flag,
            )
            .map_err(|failure| runner.raster_error(failure))?;
            runner.check_active()?;
            results.push(json!({"output":name,"report":report}));
        }
        let receipt = json!({"version":1,"frames":results});
        let reports = receipt["frames"]
            .as_array()
            .ok_or_else(|| error("画像合成結果が不正です"))?;
        if receipt["version"] != 1 || reports.len() != self.pending.len() {
            return Err(error("画像合成結果の件数または版が一致しません"));
        }
        for (pending, result) in self.pending.iter().zip(reports) {
            crate::originals::cancelled(runner.flag)?;
            let name = format!("frame-{}.png", pending.index);
            if result["output"] != name {
                return Err(error("画像合成結果の出力名が一致しません"));
            }
            let layers: Vec<_> = pending.layers.iter().collect();
            validate_report(
                &result["report"],
                runner,
                canvas,
                &layers,
                pending.index,
                pending.frame_index,
            )?;
            verify_png(&runner.directory.join(name), canvas)?;
        }
        // No cache entries or counts are committed until every result is valid.
        for (pending, result) in self.pending.iter().zip(reports) {
            write_new(
                &runner
                    .directory
                    .join(format!("report-{}.json", pending.index)),
                &serde_json::to_vec(&result["report"])?,
            )?;
        }
        crate::originals::cancelled(runner.flag)?;
        for pending in self.pending.drain(..) {
            self.frames.insert(pending.key, pending.index);
            self.stats.rendered += 1;
        }
        Ok(())
    }
}
fn verify_png(path: &Path, canvas: &Canvas) -> Result<(), RenderError> {
    if !std::fs::symlink_metadata(path)?.is_file() {
        return Err(error("描画PNGが通常ファイルではありません"));
    }
    let probe =
        sungeki_assets::probe_image(path, sungeki_assets::ProbeLimits::default()).map_err(error)?;
    if probe.format != sungeki_project::ImageFormat::Png
        || probe.width != canvas.width
        || probe.height != canvas.height
    {
        return Err(error("描画PNGの形式・寸法が計画と一致しません"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "frame_batch_tests.rs"]
mod tests;
