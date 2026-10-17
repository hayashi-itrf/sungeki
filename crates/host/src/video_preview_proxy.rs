//! Resolve only persisted, completed cache outputs belonging to this session.
use super::{guard, remaining};
use crate::{
    image_probe::video::{error, proxy_record},
    render_record::invalid,
    sessions::Sessions,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::Instant,
};
use sungeki_assets::{proxy::ProxyKey, proxy_store::read_candidate};
use sungeki_media::video_probe::{
    ProbeRuntime,
    frames::{PreparedVideo, proxy::ProxySettings},
};
use sungeki_project::{AssetProbe, VideoProbe};
use sungeki_protocol::{ApiError, VideoPreviewArgs};

pub(super) struct Selection {
    input: proxy_record::Input,
    output: proxy_record::Output,
}
impl Selection {
    pub(super) fn load(
        shared: &Arc<Mutex<Sessions>>,
        args: &VideoPreviewArgs,
        asset_id: &str,
        source: &VideoProbe,
    ) -> Result<Option<Self>, ApiError> {
        let Some(id) = &args.proxy_job_id else {
            return Ok(None);
        };
        let sessions = shared.lock().map_err(invalid)?;
        let job = proxy_record::record(&sessions, id)?;
        if job.session_id != args.session_id {
            return Err(invalid("別の作品の軽量動画は使用できません"));
        }
        let (input, state) = proxy_record::decode(&job)?;
        let frozen = input
            .project
            .assets
            .iter()
            .find(|a| a.id == asset_id)
            .ok_or_else(|| invalid("軽量動画の対象素材が一致しません"))?;
        if frozen.probe != AssetProbe::Video(source.clone()) {
            return Err(invalid("軽量動画の原本情報が現在の素材と一致しません"));
        }
        let output = state
            .completed
            .into_iter()
            .find(|o| o.asset_id == asset_id)
            .ok_or_else(|| invalid("この素材の軽量動画はまだ完成していません"))?;
        Ok(Some(Self { input, output }))
    }
    pub(super) fn identity(&self) -> (&Path, &str, &str, u64) {
        (
            &self.input.cache_root,
            &self.output.key,
            &self.output.sha256,
            self.output.byte_length,
        )
    }
    pub(super) fn manifest_path(&self) -> PathBuf {
        self.input
            .cache_root
            .join(&self.output.key)
            .join("manifest.json")
    }
    pub(super) fn open(
        &self,
        source: &VideoProbe,
        ffprobe: &Path,
        root: &Path,
        end: Instant,
        flag: &AtomicBool,
    ) -> Result<(PreparedVideo, PathBuf, guard::OriginalStamp), ApiError> {
        let settings = ProxySettings {
            max_edge: self.input.max_edge,
            encoding: self.input.encoding,
        };
        let descriptor = settings.descriptor().map_err(invalid)?;
        let key = ProxyKey::new(
            source,
            &serde_json::to_vec(&(&descriptor, &self.input.encoder_sha256)).map_err(invalid)?,
        )
        .map_err(invalid)?;
        let path = self.input.cache_root.join(key.as_str()).join("video.mov");
        let stamp = guard::OriginalStamp::read(&path)?;
        let manifest = crate::image_probe::video::cancellation::run(
            flag,
            || std::time::Instant::now() >= end,
            |cancel| read_candidate(&self.input.cache_root, &key, cancel).map_err(invalid),
        )
        .map_err(|e| guard::check(flag, end).err().unwrap_or(e))?;
        if manifest.source != *source
            || manifest.conversion != descriptor
            || manifest.encoder_sha256 != self.input.encoder_sha256
            || manifest.derived.sha256 != self.output.sha256
            || manifest.derived.byte_length != self.output.byte_length
        {
            return Err(invalid("軽量動画が生成時の記録と一致しません"));
        }
        let video = PreparedVideo::open(
            &path,
            &manifest.derived,
            &ProbeRuntime {
                ffprobe,
                root,
                timeout: remaining(end)?,
                max_source_bytes: sungeki_media::video_probe::frames::proxy::MAX_PROXY_BYTES,
            },
            flag,
        )
        .map_err(error)?;
        if video.frame_pts() != manifest.proxy_pts {
            return Err(invalid("軽量動画の時刻記録が一致しません"));
        }
        video
            .verify_proxy(source, &manifest.source_pts, settings)
            .map_err(error)?;
        guard::check(flag, end)?;
        stamp.verify(&path)?;
        Ok((video, path, stamp))
    }
}
