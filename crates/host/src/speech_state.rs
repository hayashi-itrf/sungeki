#[path = "speech_progress.rs"]
pub(crate) mod progress;
use serde::{Deserialize, Serialize};
use sungeki_jobs::Lifecycle;
use sungeki_project::{JobPhase, Progress};
use sungeki_protocol::{ApiError, ErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Execution {
    NotDispatched,
    PossiblyRunning,
    Finished,
    /// Completed locally from a verified original; no external synthesis was dispatched.
    Reused,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Artifact {
    pub path: String,
    pub sha256: String,
    pub duration_us: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u64,
    pub byte_len: u64,
    pub lip_sync: Option<sungeki_project::LipSyncAnalysis>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Application {
    NotApplicable,
    PendingConflict,
    Applied {
        revision: u32,
        overlapping_clip_ids: Vec<String>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub execution: Execution,
    pub artifact: Option<Artifact>,
    pub application: Application,
    pub error: Option<ApiError>,
}
impl State {
    pub fn queued() -> Self {
        Self {
            phase: JobPhase::Queued,
            progress: None,
            execution: Execution::NotDispatched,
            artifact: None,
            application: Application::NotApplicable,
            error: None,
        }
    }
    pub fn validate(&self) -> Result<(), ApiError> {
        Lifecycle::restore(self.phase, self.progress.clone()).map_err(invalid)?;
        if self.phase == JobPhase::Queued
            && (self.execution != Execution::NotDispatched || self.progress.is_some())
        {
            return Err(invalid("queuedの外部実行状態が不正です"));
        }
        if self.phase == JobPhase::Cancelled && self.execution == Execution::PossiblyRunning {
            return Err(invalid("外部処理未確認のまま取消完了にはできません"));
        }
        if self.phase == JobPhase::Succeeded {
            if !matches!(self.execution, Execution::Finished | Execution::Reused)
                || self.artifact.as_ref().is_none_or(|a| a.lip_sync.is_none())
                || self.error.is_some()
            {
                return Err(invalid("成功には終了確認と音声・口パク成果物が必要です"));
            }
        } else if self.artifact.is_some() || !matches!(self.application, Application::NotApplicable)
        {
            return Err(invalid("未成功ジョブの成果物/適用状態が不正です"));
        }
        if self.execution == Execution::Reused && self.phase != JobPhase::Succeeded {
            return Err(invalid("再利用音声は成功した原本としてだけ記録できます"));
        }
        if self.phase == JobPhase::Failed && self.error.is_none() {
            return Err(invalid("失敗には構造化エラーが必要です"));
        }
        if matches!(
            self.phase,
            JobPhase::Queued | JobPhase::Running | JobPhase::Cancelling
        ) && self.error.is_some()
        {
            return Err(invalid("実行中の終端エラーは許可されません"));
        }
        if let Application::Applied {
            revision,
            overlapping_clip_ids,
        } = &self.application
        {
            let mut unique = std::collections::HashSet::new();
            if *revision == 0
                || overlapping_clip_ids.iter().any(|id| {
                    uuid::Uuid::parse_str(id)
                        .ok()
                        .is_none_or(|parsed| parsed.to_string() != *id)
                        || !unique.insert(id)
                })
            {
                return Err(invalid("音声適用のrevisionまたは重なり対象IDが不正です"));
            }
        }
        if let Some(artifact) = &self.artifact {
            artifact.validate()?;
        }
        Ok(())
    }
    pub fn recover(&mut self) -> Result<bool, ApiError> {
        self.validate()?;
        if self.phase.is_terminal() {
            return Ok(false);
        }
        let life = Lifecycle::recover(self.phase, self.progress.clone()).map_err(invalid)?;
        self.phase = life.phase();
        // Retain PossiblyRunning even when the host marks its task interrupted.
        Ok(true)
    }
}
impl Artifact {
    pub(crate) fn probe(&self) -> sungeki_project::AudioProbe {
        sungeki_project::AudioProbe {
            encoding: None,
            sha256: self.sha256.clone(),
            duration_us: sungeki_project::TimeUs(self.duration_us),
            sample_rate: self.sample_rate,
            channels: self.channels,
            frames: self.frames,
            byte_length: self.byte_len,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), ApiError> {
        if !std::path::Path::new(&self.path).is_absolute()
            || self.path.len() > 4096
            || self.path.contains('\0')
        {
            return Err(invalid("音声成果物のメタデータが不正です"));
        }
        self.probe().validate().map_err(invalid)?;
        if let Some(timing) = &self.lip_sync {
            timing.validate(&self.probe()).map_err(invalid)?;
        }
        Ok(())
    }
}
pub(crate) fn invalid(error: impl std::fmt::Debug) -> ApiError {
    ApiError::new(ErrorCode::IoError, format!("音声ジョブ記録: {error:?}"))
}
