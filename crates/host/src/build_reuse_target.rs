//! Build a new candidate using captured engine metadata, without starting TTS.
//! A matching fingerprint is necessary, but original/receipt/settlement checks
//! in speech_reuse::verify are still mandatory before reusing any audio.
use crate::{build_state::invalid, speech_journal::Input};
use sungeki_project::Project;
use sungeki_protocol::ApiError;

pub(crate) fn prepare(
    source: &Input,
    revision: u32,
    project: &Project,
    clip_id: &str,
    parent_id: &str,
) -> Result<Input, ApiError> {
    let target = match source {
        Input::Local(i) => Input::Local(
            i.recapture_for_reuse(revision, project, clip_id, parent_id)
                .map_err(invalid)?,
        ),
        Input::Voicevox(i) => Input::Voicevox(
            i.recapture_for_reuse(revision, project, clip_id, parent_id)
                .map_err(invalid)?,
        ),
        Input::Sbv2(i) => Input::Sbv2(
            i.recapture_for_reuse(revision, project, clip_id, parent_id)
                .map_err(invalid)?,
        ),
        Input::Qwen(i) => Input::Qwen(
            i.recapture_for_reuse(revision, project, clip_id, parent_id)
                .map_err(invalid)?,
        ),
        Input::Irodori(i) => Input::Irodori(
            i.recapture_for_reuse(revision, project, clip_id, parent_id)
                .map_err(invalid)?,
        ),
    };
    if !super::speech_reuse::same_audio(source, &target)? {
        return Err(invalid(
            "発音文・字幕・音声設定が変更されているため再合成が必要です",
        ));
    }
    Ok(target)
}
