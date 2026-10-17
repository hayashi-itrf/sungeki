use crate::sessions::Sessions;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use sungeki_editor::{Command, EditAction, EditRequest};
use sungeki_project::{AssetLocator, PresetContent};
use sungeki_protocol::*;

fn conflict() -> ApiError {
    ApiError::new(
        ErrorCode::Conflict,
        "プリセットまたは作品が更新されています。再取得してください",
    )
}
fn replay(
    sessions: &mut Sessions,
    args: &InstantiatePresetArgs,
) -> Result<Option<SessionResponse>, ApiError> {
    let (_, session) = sessions.load(&args.session_id)?;
    let Some(recorded) = session.recorded_request(&args.operation_id) else {
        return Ok(None);
    };
    let EditAction::Apply { commands } = &recorded.action else {
        return Err(conflict());
    };
    let [
        Command::ApplyCharacterPreset {
            preset_id,
            preset_revision,
            ids,
            placement,
            ..
        },
    ] = commands.as_slice()
    else {
        return Err(conflict());
    };
    if recorded.expected_revision != args.expected_revision
        || preset_id != &args.preset_id
        || *preset_revision != args.preset_revision
        || ids != &args.ids
        || placement != &args.placement
    {
        return Err(conflict());
    }
    sessions
        .edit_session(args.session_id.clone(), recorded.clone())
        .map(Some)
}

pub fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: InstantiatePresetArgs,
) -> Result<SessionResponse, ApiError> {
    {
        let mut sessions = shared.lock().map_err(ApiError::io)?;
        if let Some(response) = replay(&mut sessions, &args)? {
            return Ok(response);
        }
        let (stored, _) = sessions.load(&args.session_id)?;
        if stored.revision != args.expected_revision {
            return Err(conflict());
        }
    }
    let SessionResponse::Preset { record } = super::presets::execute(
        shared,
        PresetRequest::Get {
            id: args.preset_id.clone(),
        },
    )?
    else {
        unreachable!()
    };
    if record.revision != args.preset_revision {
        return Err(conflict());
    }
    let preset = record.preset.ok_or_else(conflict)?;
    let PresetContent::Character { assets, .. } = &preset.content else {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "現在の適用操作はキャラクタープリセット用です。声とテンプレの適用は未接続です",
        ));
    };
    let cancel = shared
        .lock()
        .map_err(ApiError::io)?
        .profile_inspection_cancel
        .clone();
    // Verify originals without blocking the editor. No media is copied; render
    // performs its own immutable-input capture later.
    for asset in assets {
        let AssetLocator::Absolute { path } = &asset.locator else {
            unreachable!()
        };
        let measured =
            crate::original_file::inspect(Path::new(path), 4 * 1024 * 1024 * 1024, false, &cancel)?;
        if measured.sha256 != asset.probe.sha256() {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                format!("素材「{}」が保存時から変更されています", asset.name),
            ));
        }
    }
    let mut sessions = shared.lock().map_err(ApiError::io)?;
    // A concurrent identical request may have committed during file inspection.
    if let Some(response) = replay(&mut sessions, &args)? {
        return Ok(response);
    }
    sessions.edit_session(
        args.session_id,
        EditRequest {
            operation_id: args.operation_id,
            expected_revision: args.expected_revision,
            action: EditAction::Apply {
                commands: vec![Command::ApplyCharacterPreset {
                    preset_id: args.preset_id,
                    preset_revision: args.preset_revision,
                    preset,
                    ids: args.ids,
                    placement: args.placement,
                }],
            },
        },
    )
}
