//! Tool access is limited to the job's media requirements.
use crate::{RenderError, RenderRuntime, plan::error};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRuntime {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub deadline: Duration,
}
impl RenderRuntime {
    pub fn audio(&self) -> AudioRuntime {
        AudioRuntime {
            ffmpeg: self.ffmpeg.clone(),
            ffprobe: self.ffprobe.clone(),
            deadline: self.deadline,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum ExecutionRuntime<'a> {
    Video(&'a RenderRuntime),
    Audio(&'a AudioRuntime),
}
impl<'a> From<&'a RenderRuntime> for ExecutionRuntime<'a> {
    fn from(value: &'a RenderRuntime) -> Self {
        Self::Video(value)
    }
}
impl<'a> From<&'a AudioRuntime> for ExecutionRuntime<'a> {
    fn from(value: &'a AudioRuntime) -> Self {
        Self::Audio(value)
    }
}
impl<'a> ExecutionRuntime<'a> {
    pub fn ffmpeg(self) -> &'a Path {
        match self {
            Self::Video(v) => &v.ffmpeg,
            Self::Audio(v) => &v.ffmpeg,
        }
    }
    pub fn ffprobe(self) -> &'a Path {
        match self {
            Self::Video(v) => &v.ffprobe,
            Self::Audio(v) => &v.ffprobe,
        }
    }
    pub fn visual(self) -> Result<&'a RenderRuntime, RenderError> {
        match self {
            Self::Video(v) => Ok(v),
            Self::Audio(_) => Err(error("音声確認では映像描画を実行できません")),
        }
    }
}
