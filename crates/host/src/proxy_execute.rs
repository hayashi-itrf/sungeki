use super::proxy_record::{Input, Output, invalid};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use sungeki_assets::{
    proxy::ProxyKey,
    proxy_store::{ProxyManifest, publish, read_candidate},
};
use sungeki_media::video_probe::{
    ProbeRuntime,
    frames::{PreparedVideo, proxy::ProxySettings},
};
use sungeki_project::{AssetLocator, AssetProbe};
use sungeki_protocol::{ApiError, ErrorCode};
fn io(error: std::io::Error) -> ApiError {
    if error.kind() == std::io::ErrorKind::Interrupted {
        ApiError::new(ErrorCode::Cancelled, "軽量動画生成を中断しました")
    } else {
        invalid(error)
    }
}
pub(super) fn execute(input: &Input, id: &str, cancel: &AtomicBool) -> Result<Output, ApiError> {
    let ffmpeg = input.ffmpeg.canonicalize().map_err(io)?;
    let (hash, _) = sungeki_media::source_identity(&ffmpeg, 256 * 1024 * 1024, || {
        cancel.load(Ordering::Acquire)
    })
    .map_err(invalid)?;
    if hash != input.encoder_sha256 {
        return Err(ApiError::new(
            ErrorCode::ExternalModification,
            "動画変換環境が変更されました",
        ));
    }
    let asset = input
        .project
        .assets
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| invalid("動画がありません"))?;
    let AssetProbe::Video(source) = &asset.probe else {
        return Err(invalid("動画が必要です"));
    };
    let AssetLocator::Absolute { path } = &asset.locator else {
        return Err(invalid("原本の絶対パスが必要です"));
    };
    let settings = ProxySettings {
        max_edge: input.max_edge,
        encoding: input.encoding,
    };
    let descriptor = settings.descriptor().map_err(invalid)?;
    let key = ProxyKey::new(
        source,
        &serde_json::to_vec(&(&descriptor, &input.encoder_sha256)).map_err(invalid)?,
    )
    .map_err(invalid)?;
    let runtime = ProbeRuntime {
        ffprobe: &input.ffprobe,
        root: &input.work_root,
        timeout: Duration::from_secs(300),
        max_source_bytes: 4 * 1024 * 1024 * 1024,
    };
    // Generation requires a verified original even when a cache candidate exists.
    let original =
        PreparedVideo::open(Path::new(path), source, &runtime, cancel).map_err(super::error)?;
    let folder = input.cache_root.join(key.as_str());
    match std::fs::symlink_metadata(&folder) {
        Ok(_) => return reuse(input, id, &key, &descriptor, &original, &runtime, cancel),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io(error)),
    }
    let generated = original
        .generate_proxy(settings, &ffmpeg, &runtime, cancel)
        .map_err(super::error)?;
    let manifest = ProxyManifest {
        version: sungeki_assets::proxy::PROXY_FORMAT_VERSION,
        source: source.clone(),
        derived: generated.video.probe().clone(),
        conversion: descriptor.clone(),
        encoder_sha256: input.encoder_sha256.clone(),
        source_pts: original.frame_pts().to_vec(),
        proxy_pts: generated.video.frame_pts().to_vec(),
    };
    match publish(
        &input.cache_root,
        &manifest,
        generated.snapshot_path(),
        cancel,
    ) {
        Ok(_) => Ok(output(id, &key, &manifest)),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            reuse(input, id, &key, &descriptor, &original, &runtime, cancel)
        }
        Err(error) => Err(io(error)),
    }
}
fn reuse(
    input: &Input,
    id: &str,
    key: &ProxyKey,
    descriptor: &str,
    original: &PreparedVideo,
    runtime: &ProbeRuntime<'_>,
    cancel: &AtomicBool,
) -> Result<Output, ApiError> {
    let manifest = read_candidate(&input.cache_root, key, cancel).map_err(io)?;
    if manifest.source != *original.probe()
        || manifest.source_pts != original.frame_pts()
        || manifest.conversion != descriptor
        || manifest.encoder_sha256 != input.encoder_sha256
    {
        return Err(invalid("軽量動画の原本情報が一致しません"));
    }
    let path = input.cache_root.join(key.as_str()).join("video.mov");
    let video =
        PreparedVideo::open(&path, &manifest.derived, runtime, cancel).map_err(super::error)?;
    if video.frame_pts() != manifest.proxy_pts {
        return Err(invalid("軽量動画の時刻記録が一致しません"));
    }
    video
        .verify_proxy(
            original.probe(),
            original.frame_pts(),
            ProxySettings {
                max_edge: input.max_edge,
                encoding: input.encoding,
            },
        )
        .map_err(super::error)?;
    Ok(output(id, key, &manifest))
}
fn output(id: &str, key: &ProxyKey, manifest: &ProxyManifest) -> Output {
    Output {
        asset_id: id.into(),
        key: key.as_str().into(),
        sha256: manifest.derived.sha256.clone(),
        byte_length: manifest.derived.byte_length,
    }
}
