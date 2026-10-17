use crate::SessionRequest;
use sungeki_editor::{Command, EditAction};
use sungeki_project::{Clip, ClipContent};
pub(super) fn required(request: &SessionRequest) -> bool {
    let commands = match request {
        SessionRequest::Edit { edit, .. } => match &edit.action {
            EditAction::Apply { commands } => commands,
            _ => return false,
        },
        SessionRequest::PreviewEdit { commands, .. } => commands,
        _ => return false,
    };
    let wrapped = |c: &Clip| {
        matches!(
            c.content,
            ClipContent::Text {
                layout: Some(_),
                ..
            }
        )
    };
    commands.iter().any(|command| match command {
        Command::AddClip { clip } => wrapped(clip),
        Command::ReplaceProject { project, .. } => project.clips.iter().any(wrapped),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_text_layout_requires_minor_107_for_add_and_restore() {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/scene-evaluation.json"
        ))
        .unwrap();
        let mut project: sungeki_project::Project =
            serde_json::from_value(value["project"].clone()).unwrap();
        let clip = project
            .clips
            .iter_mut()
            .find(|c| matches!(c.content, ClipContent::Text { .. }))
            .unwrap();
        let ClipContent::Text { layout, .. } = &mut clip.content else {
            panic!()
        };
        *layout = Some(sungeki_project::SubtitleLayout {
            alignment: sungeki_project::TextAlignment::Center,
            wrap: true,
        });
        let add = Command::AddClip { clip: clip.clone() };
        let restore = Command::ReplaceProject {
            project,
            checkpoint_id: None,
        };
        for command in [add, restore] {
            let request: SessionRequest = serde_json::from_value(serde_json::json!({
                "type":"preview_edit", "session_id":"session", "expected_revision":0,
                "commands":[command], "offset":0, "limit":100
            }))
            .unwrap();
            assert_eq!(request.minimum_protocol_minor(), 107);
        }
    }
}

pub(super) fn edits_layout(request: &SessionRequest) -> bool {
    let commands = match request {
        SessionRequest::Edit { edit, .. } => match &edit.action {
            EditAction::Apply { commands } => commands,
            _ => return false,
        },
        SessionRequest::PreviewEdit { commands, .. } => commands,
        _ => return false,
    };
    commands
        .iter()
        .any(|c| matches!(c, Command::SetTextLayout { .. }))
}

#[cfg(test)]
#[test]
fn layout_edit_and_preview_require_minor_117() {
    let command = serde_json::json!({"type":"set_text_layout","clip_id":"text","layout":null});
    for request in [
        serde_json::json!({"type":"preview_edit","session_id":"s","expected_revision":0,"commands":[command.clone()],"offset":0,"limit":10}),
        serde_json::json!({"type":"edit","session_id":"s","edit":{"operation_id":"op","expected_revision":0,"action":{"type":"apply","commands":[command]}}}),
    ] {
        let request: SessionRequest = serde_json::from_value(request).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 117);
    }
}

pub(super) fn outlines(request: &SessionRequest) -> bool {
    let commands = match request {
        SessionRequest::Edit { edit, .. } => match &edit.action {
            EditAction::Apply { commands } => commands,
            _ => return false,
        },
        SessionRequest::PreviewEdit { commands, .. } => commands,
        _ => return false,
    };
    let outlined = |c: &Clip| {
        matches!(
            &c.content,
            ClipContent::Text {
                outline: Some(_),
                ..
            }
        )
    };
    commands.iter().any(|c| match c {
        Command::SetTextOutline { .. } => true,
        Command::AddClip { clip } => outlined(clip),
        Command::ReplaceProject { project, .. } => project.clips.iter().any(outlined),
        _ => false,
    })
}

#[cfg(test)]
#[test]
fn outline_requires_minor_118_for_edit_add_and_replace() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scene-evaluation.json"
    ))
    .unwrap();
    let mut project: sungeki_project::Project =
        serde_json::from_value(value["project"].clone()).unwrap();
    let clip = project
        .clips
        .iter_mut()
        .find(|c| matches!(c.content, ClipContent::Text { .. }))
        .unwrap();
    let ClipContent::Text { outline, .. } = &mut clip.content else {
        panic!()
    };
    *outline = Some(sungeki_project::SubtitleOutline {
        width: 3,
        color: "#ffffff".into(),
    });
    let add = Command::AddClip { clip: clip.clone() };
    for command in [
        Command::SetTextOutline {
            clip_id: clip.id.clone(),
            outline: None,
        },
        add,
        Command::ReplaceProject {
            project,
            checkpoint_id: None,
        },
    ] {
        for request in [
            serde_json::json!({"type":"preview_edit","session_id":"s","expected_revision":0,"commands":[command.clone()],"offset":0,"limit":10}),
            serde_json::json!({"type":"edit","session_id":"s","edit":{"operation_id":"op","expected_revision":0,"action":{"type":"apply","commands":[command]}}}),
        ] {
            let request: SessionRequest = serde_json::from_value(request).unwrap();
            assert_eq!(request.minimum_protocol_minor(), 118);
        }
    }
}
