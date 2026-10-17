//! Persist stage observations without changing job phase or publishing output.
use crate::{
    render_record::{decode, invalid, record, save},
    sessions::Sessions,
};
use std::sync::{Arc, Mutex};
use sungeki_project::{JobPhase, Progress};
use sungeki_protocol::ApiError;
use sungeki_render::RenderProgress;

pub(crate) fn update(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    progress: RenderProgress,
) -> Result<(), ApiError> {
    let mut sessions = shared.lock().map_err(invalid)?;
    let job = record(&sessions, id)?;
    let (_, mut state) = decode(&job)?;
    if state.phase != JobPhase::Running {
        return Ok(());
    }
    let stage = match progress {
        RenderProgress::Preparing => "素材を確認・準備中".into(),
        RenderProgress::Drawing { completed, total } => {
            format!("映像を作成中（{completed} / {total} コマ）")
        }
        RenderProgress::Audio => "音声とBGMを調整中".into(),
        RenderProgress::Encoding => "映像と音声を動画ファイルにまとめています".into(),
        RenderProgress::Verifying => "完成した動画の形式と内容を検査中".into(),
    };
    let mut lifecycle = state.lifecycle()?;
    // Counts describe only the drawing stage; do not present a guessed total %.
    lifecycle
        .update_progress(Progress {
            stage,
            fraction: None,
        })
        .map_err(invalid)?;
    state.update(lifecycle);
    save(&mut sessions, &job, &state)
}
