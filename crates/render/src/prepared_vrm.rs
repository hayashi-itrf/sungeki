use crate::{
    RenderError, RenderPlan,
    execute::Runner,
    plan::error,
    vrm_request::{VrmFrameRequest, VrmRenderRequest, VrmSource},
    vrm_worker::{VrmWorkerConfig, render_vrm_frames},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use sungeki_project::{AssetLocator, AssetProbe};
use sungeki_scene::{Layer, Primitive};
#[derive(Default)]
pub(crate) struct PreparedVrm {
    images: BTreeMap<String, String>,
}
fn key(primitive: &Primitive) -> Result<String, RenderError> {
    Ok(serde_json::to_string(primitive)?)
}
impl PreparedVrm {
    pub fn new(
        plan: &RenderPlan,
        runner: &Runner<'_>,
        resources: &mut BTreeMap<String, Value>,
    ) -> Result<Self, RenderError> {
        type Group = (String, String, u32, u32);
        let mut groups: BTreeMap<Group, BTreeMap<String, VrmFrameRequest>> = BTreeMap::new();
        for layer in &plan.scene.layers {
            if let Primitive::Vrm {
                model_asset_id,
                motion_asset_id,
                width,
                height,
                source_time_us,
                expressions,
                pose,
            } = &layer.primitive
            {
                groups
                    .entry((
                        model_asset_id.clone(),
                        motion_asset_id.clone(),
                        *width,
                        *height,
                    ))
                    .or_default()
                    .entry(key(&layer.primitive)?)
                    .or_insert_with(|| VrmFrameRequest {
                        time_us: *source_time_us,
                        expressions: expressions.clone(),
                        pose: pose.clone(),
                    });
            }
        }
        let mut prepared = Self::default();
        if groups.is_empty() {
            return Ok(prepared);
        }
        let runtime = runner
            .runtime
            .visual()?
            .vrm
            .as_ref()
            .ok_or_else(|| error("VRM描画環境が設定されていません"))?;
        let source = |id: &str| -> Result<VrmSource, RenderError> {
            let asset = plan
                .sources
                .get(id)
                .ok_or_else(|| error("VRM素材がありません"))?;
            let (AssetLocator::Absolute { path }, AssetProbe::Vrm(probe)) =
                (&asset.locator, &asset.probe)
            else {
                return Err(error("VRM原本の参照が不正です"));
            };
            Ok(VrmSource {
                path: path.into(),
                sha256: probe.sha256.clone(),
                byte_length: probe.byte_length,
            })
        };
        for (group_index, ((model, motion, width, height), frames)) in
            groups.into_iter().enumerate()
        {
            let mut frames: Vec<_> = frames.into_iter().collect();
            frames.sort_by_key(|(_, frame)| frame.time_us);
            let request = VrmRenderRequest {
                version: 1,
                model: source(&model)?,
                motion: source(&motion)?,
                width,
                height,
                frames: frames.iter().map(|(_, f)| f.clone()).collect(),
            };
            let directory = runner.directory.join(format!("vrm-{group_index}"));
            std::fs::create_dir(&directory)?;
            let timeout = runner
                .end
                .checked_duration_since(Instant::now())
                .filter(|t| !t.is_zero())
                .ok_or_else(|| error("VRM描画期限を超えました"))?
                .min(Duration::from_secs(180));
            let output = render_vrm_frames(
                &request,
                &VrmWorkerConfig {
                    node: &runtime.node,
                    worker: &runtime.worker,
                    work_root: &runtime.work_root,
                    timeout,
                },
                &directory,
                runner.flag,
            )?;
            for (index, ((key, _), image)) in frames.into_iter().zip(output).enumerate() {
                let id = format!("__vrm_frame_{group_index}_{index}");
                resources.insert(
                    id.clone(),
                    json!({"path":image.path,"sha256":image.probe.sha256,
                    "probe":AssetProbe::Image(image.probe)}),
                );
                prepared.images.insert(key, id);
            }
        }
        Ok(prepared)
    }
    pub fn resolve(&self, layers: &[&Layer]) -> Result<Vec<Layer>, RenderError> {
        layers
            .iter()
            .map(|layer| {
                let mut layer = (*layer).clone();
                if matches!(layer.primitive, Primitive::Vrm { .. }) {
                    let id = self
                        .images
                        .get(&key(&layer.primitive)?)
                        .ok_or_else(|| error("VRM画像が準備されていません"))?;
                    layer.primitive = Primitive::Image {
                        asset_id: id.clone(),
                    };
                }
                Ok(layer)
            })
            .collect()
    }
}
