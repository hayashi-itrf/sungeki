//! Called on the host request blocking worker. Original I/O stays outside the
//! session lock. Replays compare saved provenance without recapturing it.
use crate::{
    build_record::{self as build, continuation::Continuation},
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex};
use sungeki_protocol::{ApiError, StartBuildVideoArgs};

pub(super) fn capture(
    shared: &Arc<Mutex<Sessions>>,
    args: &StartBuildVideoArgs,
) -> Result<Option<Continuation>, ApiError> {
    let source = {
        let sessions = shared.lock().map_err(invalid)?;
        if let Some(existing) = sessions
            .store
            .load_job(&args.job_id)
            .map_err(storage_error)?
        {
            let (input, _) = build::decode(&existing)?;
            match (&input.continuation, &args.reuse_from) {
                (None, None) => {}
                (Some(saved), Some(request)) if saved.matches_request(request) => {}
                _ => return Err(invalid("同じ制作IDの再利用指定が変更されています")),
            }
            return Ok(input.continuation);
        }
        let Some(request) = &args.reuse_from else {
            return Ok(None);
        };
        if request.parent_job_id == args.job_id
            || request.speech.is_empty()
            || request.speech.len() > 200
        {
            return Err(invalid("新しい制作IDと1〜200件の再利用対応が必要です"));
        }
        sessions
            .store
            .load_job(&request.parent_job_id)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("再利用元の制作がありません"))?
    };
    let request = args
        .reuse_from
        .as_ref()
        .ok_or_else(|| invalid("再利用指定がありません"))?;
    let (plan, _) = build::decode(&source)?;
    let count = args
        .recipe
        .scenes
        .iter()
        .map(|s| s.lines.len())
        .sum::<usize>() as u16;
    let mut result: Option<Continuation> = None;
    for (target_key, &source_index) in &request.speech {
        let target_index: u16 = target_key
            .parse()
            .map_err(|_| invalid("再利用先の発話番号が不正です"))?;
        if target_key != &target_index.to_string() {
            return Err(invalid(
                "再利用先の発話番号は0始まりの整数で指定してください",
            ));
        }
        if target_index >= count || source_index >= plan.speech_count {
            return Err(invalid("再利用する発話番号が範囲外です"));
        }
        let id = &plan.step_ids[usize::from(source_index) + 2];
        let input = {
            let sessions = shared.lock().map_err(invalid)?;
            let child = sessions
                .store
                .load_job(id)
                .map_err(storage_error)?
                .ok_or_else(|| invalid("指定された発話の成功記録がありません"))?;
            crate::speech_journal::decode_input(&child)?.0
        };
        let proof = build::speech_reuse::verify_blocking(shared, &source.id, id, &input)?;
        let next = Continuation::from_verified(&[(target_index, proof)], count)?;
        result = Some(match result {
            Some(prior) => prior.merge(next, count)?,
            None => next,
        });
    }
    Ok(result)
}
