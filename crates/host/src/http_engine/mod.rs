//! Host-owned HTTP engines. External servers are never adopted or terminated.
mod launch;
use crate::{sbv2_process::OwnedProcess, sessions::Sessions};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use sungeki_protocol::{ApiError, ErrorCode};
use tokio::sync::Mutex as AsyncMutex;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Engine {
    Voicevox,
    Aivis,
    Coeiroink,
}
impl Engine {
    pub fn local(engine: sungeki_project::LocalSpeechEngine) -> Self {
        match engine {
            sungeki_project::LocalSpeechEngine::AivisSpeech => Self::Aivis,
            sungeki_project::LocalSpeechEngine::Coeiroink => Self::Coeiroink,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Voicevox => "voicevox",
            Self::Aivis => "aivis",
            Self::Coeiroink => "coeiroink",
        }
    }
}
struct Running {
    endpoint: String,
    process: OwnedProcess,
}
static OWNED: AsyncMutex<Option<HashMap<Engine, Running>>> = AsyncMutex::const_new(None);
static STOPPING: AtomicBool = AtomicBool::new(false);
fn error(message: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::EngineUnavailable, message.to_string())
}
fn cancelled(flag: &AtomicBool) -> Result<(), ApiError> {
    if flag.load(Ordering::Acquire) || STOPPING.load(Ordering::Acquire) {
        Err(ApiError::new(
            ErrorCode::Cancelled,
            "音声エンジンの起動を取り消しました",
        ))
    } else {
        Ok(())
    }
}
pub(crate) async fn ensure(
    shared: &Arc<Mutex<Sessions>>,
    engine: Engine,
    endpoint: &str,
    cancel: &AtomicBool,
) -> Result<(), ApiError> {
    cancelled(cancel)?;
    let database = shared.lock().map_err(error)?.review_database_path.clone();
    let settings = crate::runtime_settings::load(&database)?;
    let executable = match engine {
        Engine::Voicevox => settings.voicevox_engine.clone(),
        Engine::Aivis => settings.aivis_engine.clone(),
        Engine::Coeiroink => settings.coeiroink_engine.clone(),
    };
    let Some(executable) = executable else {
        return Ok(());
    };
    let endpoint = sungeki_speech::voicevox::canonical_endpoint(endpoint).map_err(error)?;
    let mut owned = OWNED.lock().await;
    cancelled(cancel)?;
    let engines = owned.get_or_insert_with(HashMap::new);
    if let Some(running) = engines.get_mut(&engine) {
        if !running.process.has_exited().map_err(error)? {
            if running.endpoint != endpoint {
                return Err(error(
                    "このエンジンは別の接続先で起動中です。Sungekiを終了してから接続先を変更してください",
                ));
            }
            return Ok(());
        }
        running
            .process
            .stop(Duration::from_secs(5))
            .map_err(error)?;
        engines.remove(&engine);
    }
    // An occupied port belongs to somebody else. The normal catalog/synthesis
    // identity checks will accept or reject it; never signal its owner.
    let command = match launch::command(engine, &endpoint, &executable, &settings)? {
        Some(c) => c,
        None => return Ok(()),
    };
    cancelled(cancel)?;
    let mut process = OwnedProcess::spawn(command).map_err(error)?;
    let result = wait(&process, engine, &endpoint, cancel).await;
    if let Err(cause) = result {
        if let Err(cleanup) = process.stop(Duration::from_secs(5)) {
            engines.insert(engine, Running { endpoint, process });
            return Err(error(format!(
                "{} / 停止確認に失敗: {}",
                cause.message, cleanup
            )));
        }
        return Err(cause);
    }
    engines.insert(engine, Running { endpoint, process });
    Ok(())
}
async fn wait(
    process: &OwnedProcess,
    engine: Engine,
    endpoint: &str,
    cancel: &AtomicBool,
) -> Result<(), ApiError> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(600);
    loop {
        cancelled(cancel)?;
        if process.has_exited().map_err(error)? {
            return Err(error(
                "音声エンジンが起動中に終了しました。実行ファイルとエンジンのログを確認してください",
            ));
        }
        if healthy(engine, endpoint).await {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(error("音声エンジンが600秒以内に起動しませんでした"));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
async fn healthy(engine: Engine, endpoint: &str) -> bool {
    let flag = AtomicBool::new(false);
    if engine == Engine::Voicevox {
        let Ok(client) =
            sungeki_speech::voicevox::VoicevoxDiscovery::new(endpoint, Duration::from_secs(1))
        else {
            return false;
        };
        client.engine(&flag).await.is_ok()
    } else {
        let kind = if engine == Engine::Aivis {
            sungeki_project::LocalSpeechEngine::AivisSpeech
        } else {
            sungeki_project::LocalSpeechEngine::Coeiroink
        };
        let Ok(client) = sungeki_speech::local_http::LocalSpeechClient::new(
            endpoint,
            kind,
            Duration::from_secs(1),
        ) else {
            return false;
        };
        client.catalog(&flag).await.is_ok()
    }
}
pub(crate) fn begin_shutdown() {
    STOPPING.store(true, Ordering::Release);
}
pub(crate) async fn shutdown() {
    let mut owned = OWNED.lock().await;
    if let Some(engines) = owned.as_mut() {
        for running in engines.values_mut() {
            while let Err(e) = running.process.stop(Duration::from_secs(5)) {
                eprintln!("HTTP engine shutdown: {e}");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
        engines.clear();
    }
}

#[cfg(test)]
mod tests;
