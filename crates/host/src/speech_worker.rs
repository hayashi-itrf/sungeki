//! Dispatch choice is captured when a job is scheduled, never selected from
//! changing settings halfway through the queue. Project edits remain shared.
use crate::{sbv2_originals::Binding, sessions::Sessions, speech_journal::Input};
use std::{
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::Duration,
};
use sungeki_protocol::{ApiError, ErrorCode};
fn invalid(message: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message.to_string())
}

pub(crate) enum Worker {
    Local,
    Voicevox,
    Irodori {
        binding: crate::irodori_originals::binding::Binding,
        port: u16,
    },
    Qwen {
        binding: crate::qwen_originals::binding::Binding,
        port: u16,
    },
    Sbv2 {
        binding: Binding,
        cpu_only: bool,
    },
}
impl Worker {
    /// Pure configuration checks, before creating a new durable job. Original
    /// files are inspected later under the execution lease, off the IPC thread.
    pub fn validate(&self, input: &Input) -> Result<(), ApiError> {
        match (self, input) {
            (Self::Local, Input::Local(_)) => Ok(()),
            (Self::Voicevox, Input::Voicevox(_)) => Ok(()),
            (Self::Irodori { port, .. }, Input::Irodori(_)) => {
                if *port < 1024 {
                    return Err(invalid("Irodoriの接続先が不正です"));
                }
                Ok(())
            }
            (Self::Qwen { port, .. }, Input::Qwen(input)) => {
                if *port < 1024 || input.endpoint() != format!("http://127.0.0.1:{port}/") {
                    return Err(invalid("Qwenの起動設定とジョブの接続先が一致しません"));
                }
                Ok(())
            }
            (Self::Sbv2 { binding, .. }, Input::Sbv2(input)) => {
                binding.managed_address()?;
                let endpoint =
                    sungeki_speech::sbv2::canonical_endpoint(&binding.endpoint).map_err(invalid)?;
                if endpoint != input.endpoint() {
                    return Err(invalid("SBV2の起動設定とジョブの接続先が一致しません"));
                }
                Ok(())
            }
            _ => Err(invalid("音声ジョブと実行エンジンが一致しません")),
        }
    }
    pub async fn run(
        self,
        shared: &Arc<Mutex<Sessions>>,
        id: &str,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), ApiError> {
        match self {
            Self::Local => crate::local_speech::run(shared, id, &cancel).await,
            Self::Irodori { binding, port } => {
                crate::irodori_originals::job::run(
                    shared,
                    id,
                    binding,
                    port,
                    cancel,
                    Duration::from_secs(600),
                )
                .await
            }
            Self::Qwen { binding, port } => {
                crate::qwen_originals::job::run(
                    shared,
                    id,
                    binding,
                    port,
                    cancel,
                    Duration::from_secs(120),
                )
                .await
            }
            Self::Voicevox => {
                crate::speech_output::run(shared, id, &cancel, Duration::from_secs(120)).await
            }
            Self::Sbv2 { binding, cpu_only } => {
                crate::sbv2_exchange::run(
                    shared,
                    id,
                    binding,
                    cpu_only,
                    cancel,
                    Duration::from_secs(120),
                )
                .await
            }
        }
    }
}
