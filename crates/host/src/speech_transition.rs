use crate::speech_state::{Application, Execution, State, invalid};
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;

pub(crate) fn validate(old: &State, new: &State) -> Result<(), ApiError> {
    use Execution::*;
    use JobPhase::*;
    let phase_ok = old.phase == new.phase
        || matches!(
            (old.phase, new.phase),
            (Queued, Running | Cancelling | Interrupted)
                | (Running, Cancelling | Succeeded | Failed | Interrupted)
                | (Cancelling, Cancelled | Interrupted)
        );
    if !phase_ok {
        return Err(invalid("音声ジョブの状態を逆戻りできません"));
    }
    if old.execution != new.execution
        && !matches!(
            (old.execution, new.execution),
            (NotDispatched, PossiblyRunning) | (PossiblyRunning, Finished)
        )
    {
        return Err(invalid("外部実行の終了確認を省略/取り消しできません"));
    }
    if old.execution == NotDispatched && new.execution == PossiblyRunning && new.phase != Running {
        return Err(invalid("実行開始後だけ外部要求を送信できます"));
    }
    if old.phase.is_terminal() {
        let mut permitted = old.clone();
        permitted.execution = new.execution;
        let same_application = serde_json::to_vec(&old.application).map_err(invalid)?
            == serde_json::to_vec(&new.application).map_err(invalid)?;
        if !same_application
            && !matches!(
                (&old.application, &new.application),
                (
                    Application::PendingConflict,
                    Application::Applied { .. } | Application::NotApplicable
                )
            )
        {
            return Err(invalid("音声成果物の適用状態を逆戻りできません"));
        }
        permitted.application = new.application.clone();
        if serde_json::to_vec(&permitted).map_err(invalid)?
            != serde_json::to_vec(new).map_err(invalid)?
        {
            return Err(invalid("終端ジョブの結果を上書きできません"));
        }
    }
    Ok(())
}
