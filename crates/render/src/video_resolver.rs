//! Resolve temporal primitives before the static compositor/cache. Resources
//! remain valid until the next resolve call; consume each result sequentially.
#[path = "video_decoded_cache.rs"]
mod decoded_cache;
use crate::{
    RenderError, VideoFrameResource, originals::cancelled, plan::error, stage_video_frame,
};
use decoded_cache::DecodedCache;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use sungeki_media::video_probe::frames::PreparedVideo;
use sungeki_scene::{Layer, Primitive};
pub struct ResolvedVideoLayers {
    pub layers: Vec<Layer>,
    pub resources: BTreeMap<String, VideoFrameResource>,
}
pub struct VideoLayerResolver {
    videos: BTreeMap<String, PreparedVideo>,
    previous: Vec<PathBuf>,
    sequence: u64,
    decoded: DecodedCache,
    requested: BTreeMap<String, BTreeSet<usize>>,
}
impl VideoLayerResolver {
    pub fn new(videos: BTreeMap<String, PreparedVideo>) -> Self {
        Self {
            videos,
            previous: Vec::new(),
            sequence: 0,
            decoded: DecodedCache::default(),
            requested: BTreeMap::new(),
        }
    }
    /// Register the immutable job's actual visual requests for bounded read-ahead.
    pub fn plan_frames(&mut self, layers: &[Layer]) -> Result<(), RenderError> {
        let mut requested: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
        if layers.len() > 100_000 {
            return Err(error("動画frame要求が上限を超えています"));
        }
        for layer in layers {
            if let Primitive::Video {
                asset_id,
                source_time_us,
            } = &layer.primitive
            {
                let video = self
                    .videos
                    .get(asset_id)
                    .ok_or_else(|| error("動画原本が準備されていません"))?;
                if let Some(index) = video.frame_index_at(*source_time_us).map_err(error)? {
                    requested.entry(asset_id.clone()).or_default().insert(index);
                }
            }
        }
        self.requested = requested;
        Ok(())
    }
    /// The directory must be private and owned by the render job. Existing files
    /// are never overwritten. Call only after the preceding frame was consumed.
    pub fn resolve(
        &mut self,
        layers: &[&Layer],
        directory: &Path,
        ffmpeg: &Path,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<ResolvedVideoLayers, RenderError> {
        cancelled(cancel)?;
        if !directory.is_absolute() || timeout.is_zero() || timeout > Duration::from_secs(300) {
            return Err(error("動画frame解決の実行条件が不正です"));
        }
        // These paths were created exclusively by this resolver in job staging.
        // The caller must not replace or move files in its private directory.
        for path in self.previous.drain(..) {
            std::fs::remove_file(path)?;
        }
        let end = Instant::now() + timeout;
        let mut result = ResolvedVideoLayers {
            layers: Vec::new(),
            resources: BTreeMap::new(),
        };
        let mut resolved: BTreeMap<(String, u64), Option<String>> = BTreeMap::new();
        let mut bytes = 0u64;
        for layer in layers {
            cancelled(cancel)?;
            let mut layer = (*layer).clone();
            let Primitive::Video {
                asset_id,
                source_time_us,
            } = &layer.primitive
            else {
                result.layers.push(layer);
                continue;
            };
            let lookup = (asset_id.clone(), source_time_us.0);
            let id = if let Some(id) = resolved.get(&lookup) {
                id.clone()
            } else {
                let video = self
                    .videos
                    .get(asset_id)
                    .ok_or_else(|| error("動画原本が準備されていません"))?;
                let remaining = end
                    .checked_duration_since(Instant::now())
                    .filter(|d| !d.is_zero())
                    .ok_or_else(|| error("動画frame解決の実行期限を超えました"))?;
                let frame = if let Some(index) =
                    video.frame_index_at(*source_time_us).map_err(error)?
                {
                    let key = (asset_id.clone(), index);
                    if let Some(frame) = self.decoded.get(&key) {
                        Some(frame)
                    } else {
                        let stream = &video.probe().metadata.video;
                        let frame_bytes = u64::from(stream.width) * u64::from(stream.height) * 4;
                        let range = decoded_cache::batch_range(
                            index,
                            frame_bytes,
                            self.requested.get(asset_id),
                        );
                        // Do not publish tentative callback output into the cache.
                        let mut frames = Vec::new();
                        video
                            .decode_range(range, ffmpeg, remaining, cancel, |frame| {
                                frames.push(frame);
                                Ok(())
                            })
                            .map_err(error)?;
                        for frame in frames {
                            self.decoded
                                .insert((asset_id.clone(), frame.frame_index), frame);
                        }
                        self.decoded.get(&key)
                    }
                } else {
                    None
                };
                let id = if let Some(frame) = frame {
                    self.sequence = self
                        .sequence
                        .checked_add(1)
                        .ok_or_else(|| error("動画frame番号上限"))?;
                    let path = directory.join(format!("resolved-video-{}.rgba", self.sequence));
                    let resource = stage_video_frame(frame, &path, cancel)?;
                    self.previous.push(path);
                    bytes += u64::from(resource.decoded_video.width)
                        * u64::from(resource.decoded_video.height)
                        * 4;
                    if bytes > 1024 * 1024 * 1024 {
                        return Err(error("同時動画frameの合計が1GiBを超えています"));
                    }
                    // Content and dimensions are part of the image identity used
                    // by FrameCache. Original asset ID alone is insufficient.
                    let id = format!(
                        "video-frame:{}:{}x{}",
                        resource.sha256,
                        resource.decoded_video.width,
                        resource.decoded_video.height
                    );
                    result.resources.insert(id.clone(), resource);
                    Some(id)
                } else {
                    None
                };
                resolved.insert(lookup, id.clone());
                id
            };
            if let Some(asset_id) = id {
                layer.primitive = Primitive::Image { asset_id };
                result.layers.push(layer);
            }
        }
        cancelled(cancel)?;
        if Instant::now() >= end {
            return Err(error("動画frame解決の実行期限を超えました"));
        }
        Ok(result)
    }
}
