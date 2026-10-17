use crate::{RenderError, RenderPlan, plan::error};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use sungeki_project::{AssetLocator, AssetProbe};

pub(crate) fn cancelled(flag: &AtomicBool) -> Result<(), RenderError> {
    if flag.load(Ordering::Acquire) {
        Err(RenderError::Cancelled)
    } else {
        Ok(())
    }
}
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), RenderError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
pub(crate) fn read_verified(
    path: &Path,
    hash: &str,
    expected_len: Option<u64>,
) -> Result<Vec<u8>, RenderError> {
    if !path.is_absolute() {
        return Err(error("原本は絶対パスが必要です"));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let before = file.metadata()?;
    if !before.is_file()
        || before.len() == 0
        || before.len() > 64 * 1024 * 1024
        || expected_len.is_some_and(|size| size != before.len())
    {
        return Err(RenderError::SourceChanged(
            "原本の種類またはサイズが変わっています".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    Read::by_ref(&mut file)
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let current = fs::symlink_metadata(path)?;
    let stamp = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    if !current.is_file()
        || stamp(&before) != stamp(&after)
        || stamp(&after) != stamp(&current)
        || bytes.len() as u64 != before.len()
        || format!("{:x}", Sha256::digest(&bytes)) != hash
    {
        return Err(RenderError::SourceChanged(
            "原本が登録時の内容から変更・置換されています".into(),
        ));
    }
    Ok(bytes)
}

pub(crate) fn capture(
    plan: &RenderPlan,
    runner: &crate::execute::Runner<'_>,
) -> Result<BTreeMap<String, Value>, RenderError> {
    let directory = runner.directory;
    let flag = runner.flag;
    let mut resources = BTreeMap::new();
    for (index, (id, asset)) in plan.sources.iter().enumerate() {
        cancelled(flag)?;
        let AssetLocator::Absolute { path } = &asset.locator else {
            return Err(error("出力前に素材を絶対パスへ解決してください"));
        };
        let target = directory.join(format!("source-{index}"));
        let mut decoded_audio_path = None;
        match &asset.probe {
            AssetProbe::Vrm(_) => continue, // Captured and hash-checked by the VRM worker.
            AssetProbe::Video(_) => continue, // Captured by prepare::visuals as PreparedVideo.
            AssetProbe::Psd(expected) => {
                let bytes = read_verified(
                    Path::new(path),
                    &expected.sha256,
                    Some(expected.byte_length),
                )?;
                write_new(&target, &bytes)?;
            }
            AssetProbe::Image(expected) => {
                let actual = sungeki_assets::snapshot_image(
                    Path::new(path),
                    &target,
                    &expected.sha256,
                    sungeki_assets::ProbeLimits::default(),
                )
                .map_err(error)?;
                if &actual != expected {
                    return Err(error("画像の検査記録が原本と一致しません"));
                }
            }
            AssetProbe::Audio(expected) => {
                let bytes = read_verified(
                    Path::new(path),
                    &expected.sha256,
                    Some(expected.byte_length),
                )?;
                write_new(&target, &bytes)?;
                let timeout = runner
                    .end
                    .checked_duration_since(std::time::Instant::now())
                    .filter(|t| !t.is_zero())
                    .ok_or_else(|| error("音声原本の準備期限を超えました"))?
                    .min(std::time::Duration::from_secs(3600));
                let runtime = sungeki_media::AudioDecodeRuntime {
                    ffmpeg: runner.runtime.ffmpeg(),
                    root: directory,
                    timeout,
                };
                let decode_error = |cause: sungeki_media::AudioDecodeError| match &cause {
                    sungeki_media::AudioDecodeError::Mismatch
                    | sungeki_media::AudioDecodeError::Source(
                        sungeki_media::WaveformCacheError::SourceChanged,
                    ) => RenderError::SourceChanged(cause.to_string()),
                    _ => error(cause),
                };
                if expected.encoding.is_some() {
                    let audio =
                        sungeki_media::decode_mp3_original_file(&target, expected, &runtime, flag)
                            .map_err(decode_error)?;
                    let decoded = directory.join(format!("decoded-source-{index}.wav"));
                    // Both files belong to this job on the same filesystem.
                    // Linking is atomic and refuses an existing destination;
                    // dropping audio removes only its private temporary name.
                    fs::hard_link(audio.path(), &decoded)?;
                    decoded_audio_path = Some(decoded);
                } else {
                    sungeki_media::decode_audio_original(&target, expected, None, flag)
                        .map_err(decode_error)?;
                }
            }
        }
        resources.insert(
            id.clone(),
            json!({"path": target, "sha256": asset.probe.sha256(), "probe": asset.probe, "decoded_audio_path": decoded_audio_path}),
        );
    }
    Ok(resources)
}

impl RenderPlan {
    /// Check current source bytes without producing render files. This verifies
    /// hashes and byte lengths, not decoder metadata or visual/audio quality.
    /// Rendering must capture and verify the originals again before using them.
    pub fn verify_original_hashes(&self) -> Result<usize, RenderError> {
        for asset in self.sources.values() {
            let AssetLocator::Absolute { path } = &asset.locator else {
                return Err(error("検査前に素材を絶対パスへ解決してください"));
            };
            let size = match &asset.probe {
                AssetProbe::Image(p) => p.byte_length,
                AssetProbe::Audio(p) => p.byte_length,
                AssetProbe::Psd(p) => p.byte_length,
                AssetProbe::Video(p) => p.byte_length,
                AssetProbe::Vrm(p) => p.byte_length,
            };
            let limit = if matches!(asset.probe, AssetProbe::Video(_)) {
                4 * 1024 * 1024 * 1024
            } else if matches!(asset.probe, AssetProbe::Vrm(_)) {
                100 * 1024 * 1024
            } else {
                64 * 1024 * 1024
            };
            if size == 0 || size > limit {
                return Err(error("原本の検査容量が上限を超えています"));
            }
            let identity = sungeki_media::source_identity(Path::new(path), size, || false)
                .map_err(|cause| match cause {
                    sungeki_media::WaveformCacheError::SourceChanged
                    | sungeki_media::WaveformCacheError::Wav(sungeki_media::WavError::TooLarge) => {
                        RenderError::SourceChanged(cause.to_string())
                    }
                    _ => error(cause),
                })?;
            if identity.0 != asset.probe.sha256() || identity.1 != size {
                return Err(RenderError::SourceChanged(
                    "原本が登録時の内容から変更されています".into(),
                ));
            }
        }
        Ok(self.sources.len())
    }
}
