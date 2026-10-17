use crate::{Check, CheckKind, CheckStatus, Issue, Severity};
use std::{
    fs,
    path::{Path, PathBuf},
};
use sungeki_assets::{ProbeError, ProbeLimits, SnapshotError, snapshot_image};
use sungeki_project::{AssetLocator, ImageProbe, Project};

pub struct CapturedImage {
    pub asset_id: String,
    pub path: PathBuf,
    pub probe: ImageProbe,
}
/// Keeps the private source copies alive for subsequent diagnostics. Dropping the
/// result removes only the new directory this invocation successfully reserved.
pub struct ImageReview {
    pub check: Check,
    pub issues: Vec<Issue>,
    pub originals: Vec<CapturedImage>,
    directory: Option<PathBuf>,
}
impl ImageReview {
    /// Report cleanup errors before publishing a completed report. Drop remains
    /// a fallback for unwinding, not evidence that cleanup succeeded.
    pub fn cleanup(&mut self) -> std::io::Result<()> {
        if let Some(directory) = &self.directory {
            match fs::remove_dir_all(directory) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            self.directory = None;
        }
        Ok(())
    }
}
impl Drop for ImageReview {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("review image cleanup failed: {error}");
        }
    }
}
#[derive(Debug)]
pub enum ImageReviewError {
    Io(std::io::Error),
    Cancelled,
}

/// Inspect all registered images against their registered content, without
/// changing Project. The caller supplies a new absolute, private job directory.
/// Cancellation is checked between bounded image reads, not within the decoder.
/// max_total_bytes bounds the encoded copies; decoded memory uses ProbeLimits.
pub fn review_images(
    project: &Project,
    directory: &Path,
    max_total_bytes: u64,
    cancelled: impl Fn() -> bool,
) -> Result<ImageReview, ImageReviewError> {
    if !directory.is_absolute() {
        return Err(ImageReviewError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "検査用フォルダーには絶対パスが必要です",
        )));
    }
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(directory).map_err(ImageReviewError::Io)?;
    let mut result = ImageReview {
        check: Check {
            kind: CheckKind::AssetIntegrity,
            status: CheckStatus::Passed,
            description: "登録された画像の内容と読み取りを確認しました。検査後の原本変更や映像の見た目は別途確認が必要です。".into(),
        },
        issues: Vec::new(), originals: Vec::new(), directory: Some(directory.into()),
    };
    let mut remaining = max_total_bytes;
    for (index, asset) in project.assets.iter().enumerate() {
        if cancelled() {
            result.cleanup().map_err(ImageReviewError::Io)?;
            return Err(ImageReviewError::Cancelled);
        }
        let sungeki_project::AssetProbe::Image(expected) = &asset.probe else {
            result.check.status = CheckStatus::NotRun;
            result.check.description =
                "画像の内容を検査しました。音声の検査範囲は音声項目を、PSD・動画は追加説明と指摘欄を参照してください。".into();
            continue;
        };
        // File names are local indices, never untrusted IDs or source path pieces.
        let destination = directory.join(format!("{index}.original"));
        let limits = ProbeLimits {
            max_file_bytes: ProbeLimits::default()
                .max_file_bytes
                .min(expected.byte_length),
            ..ProbeLimits::default()
        };
        let capture =
            if expected.byte_length > remaining || expected.byte_length > limits.max_file_bytes {
                Err(SnapshotError::Source(ProbeError::TooLarge))
            } else {
                // Failed reads still consume their reservation; subsequent sources
                // cannot repeatedly spend the same scan budget.
                remaining -= expected.byte_length;
                match &asset.locator {
                    AssetLocator::Absolute { path } => {
                        // Growth already proves a mismatch. Do not increase the
                        // registered-byte read budget just to identify that change.
                        if fs::metadata(path)
                            .is_ok_and(|m| m.is_file() && m.len() > expected.byte_length)
                        {
                            Err(SnapshotError::Source(ProbeError::SourceMismatch))
                        } else {
                            snapshot_image(Path::new(path), &destination, &expected.sha256, limits)
                        }
                    }
                    _ => Err(SnapshotError::Source(ProbeError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "この素材の参照方法はまだ検査できません",
                    )))),
                }
            };
        match capture {
            Ok(probe)
                if probe == *expected
                    && asset.content_hash.as_deref() == Some(probe.sha256.as_str()) =>
            {
                result.originals.push(CapturedImage {
                    asset_id: asset.id.clone(),
                    path: destination,
                    probe,
                });
            }
            outcome => {
                let (code, message) = match outcome {
                    Ok(_) => (
                        "ASSET_METADATA_MISMATCH",
                        "画像の実際の情報が登録内容と一致しません。素材を再指定してください".into(),
                    ),
                    Err(SnapshotError::Source(ProbeError::SourceMismatch)) => (
                        "ASSET_CHANGED",
                        "画像が登録時から変更されています。素材を再指定してください".into(),
                    ),
                    Err(SnapshotError::Source(ProbeError::Io(e)))
                        if e.kind() == std::io::ErrorKind::NotFound =>
                    {
                        ("ASSET_MISSING", "画像ファイルが見つかりません".into())
                    }
                    Err(error) => ("ASSET_CAPTURE_FAILED", error.to_string()),
                };
                result.issues.push(Issue {
                    severity: Severity::Error,
                    code: code.into(),
                    field: format!("/assets/{index}"),
                    asset_id: Some(asset.id.clone()),
                    clip_id: None,
                    scene_id: None,
                    start_us: None,
                    end_us: None,
                    message,
                });
            }
        }
    }
    if cancelled() {
        result.cleanup().map_err(ImageReviewError::Io)?;
        return Err(ImageReviewError::Cancelled);
    }
    if !result.issues.is_empty() {
        result.check.status = CheckStatus::Failed;
        result.check.description = "画像の欠落・変更、または検査用原本の確保に問題があります。指摘された素材を確認してください。".into();
    } else if project.assets.is_empty() {
        result.check.description = "登録された画像はありません（検査対象なし）。".into();
    }
    Ok(result)
}
