use crate::{Command, History, HistoryError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use sungeki_project::{Project, ValidationError};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EditAction {
    Apply { commands: Vec<Command> },
    Undo { expected_operation_id: String },
    Redo { expected_operation_id: String },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct EditRequest {
    pub operation_id: String,
    pub expected_revision: u32,
    pub action: EditAction,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct EditReceipt {
    pub operation_id: String,
    pub revision: u32,
    pub no_op: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub created_ids: Option<BTreeMap<String, String>>,
    /// Absent only on receipts recorded before edit outcome reporting existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub changed_ids: Option<BTreeMap<String, Vec<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub summary: Option<crate::EditSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub diagnostics: Option<sungeki_project::Diagnostics>,
}
#[derive(Debug)]
pub enum OperationError {
    InvalidId,
    ReusedId,
    WrongUndoTarget,
    History(HistoryError),
    InvalidSnapshot(String),
}
impl From<HistoryError> for OperationError {
    fn from(error: HistoryError) -> Self {
        Self::History(error)
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Recorded {
    request: EditRequest,
    receipt: EditReceipt,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct EditSession {
    history: History,
    receipts: BTreeMap<String, Recorded>,
}
impl EditSession {
    pub fn new(project: Project) -> Result<Self, ValidationError> {
        Ok(Self {
            history: History::new(project)?,
            receipts: BTreeMap::new(),
        })
    }
    pub fn recorded_request(&self, operation_id: &str) -> Option<&EditRequest> {
        self.receipts
            .get(operation_id)
            .map(|record| &record.request)
    }
    pub fn history(&self) -> &History {
        &self.history
    }
    /// Host-only save bookkeeping. Keep the original request/receipt journal
    /// intact so retries still compare with exactly the caller's original input.
    /// Persist this candidate, checkpoint relocations and binding atomically.
    pub fn relocate_audio(
        &mut self,
        expected_revision: u32,
        relocations: &crate::AudioRelocations,
    ) -> Result<bool, OperationError> {
        Ok(self
            .history
            .relocate_audio(expected_revision, relocations)?)
    }
    // Call on a clone and commit storage before replacing the host's state.
    pub fn execute(&mut self, request: EditRequest) -> Result<EditReceipt, OperationError> {
        let id =
            uuid::Uuid::parse_str(&request.operation_id).map_err(|_| OperationError::InvalidId)?;
        if id.hyphenated().to_string() != request.operation_id {
            return Err(OperationError::InvalidId);
        }
        if let Some(record) = self.receipts.get(&request.operation_id) {
            return if record.request == request {
                Ok(record.receipt.clone())
            } else {
                Err(OperationError::ReusedId)
            };
        }
        let before = self.history.project().clone();
        let mut created_ids = BTreeMap::new();
        let no_op = match &request.action {
            EditAction::Apply { commands } => {
                if self.history.revision() != request.expected_revision {
                    return Err(HistoryError::Conflict.into());
                }
                let plan = if crate::has_batch_references(commands) {
                    let allocations = crate::reference_allocation::allocate(
                        self.history.project(),
                        request.expected_revision,
                        commands,
                    )
                    .map_err(HistoryError::Validation)?;
                    Some(
                        crate::plan_batch_references(
                            self.history.project(),
                            commands,
                            &allocations,
                        )
                        .map_err(HistoryError::Validation)?,
                    )
                } else {
                    None
                };
                let resolved = if let Some(plan) = &plan {
                    &plan.commands
                } else {
                    commands
                };
                let changed = self.history.apply(request.expected_revision, resolved)?;
                if let Some(plan) = plan {
                    created_ids = plan.created_ids;
                }
                if changed {
                    self.history.tag_latest(&request.operation_id);
                }
                !changed
            }
            EditAction::Undo {
                expected_operation_id,
            } => {
                if self.history.undo_operation_id() != Some(expected_operation_id.as_str()) {
                    return Err(OperationError::WrongUndoTarget);
                }
                self.history.undo(request.expected_revision)?;
                false
            }
            EditAction::Redo {
                expected_operation_id,
            } => {
                if self.history.redo_operation_id() != Some(expected_operation_id.as_str()) {
                    return Err(OperationError::WrongUndoTarget);
                }
                self.history.redo(request.expected_revision)?;
                false
            }
        };
        let (changed_ids, summary) = crate::edit_outcome::outcome(&before, self.history.project());
        let receipt = EditReceipt {
            operation_id: request.operation_id.clone(),
            revision: self.history.revision(),
            no_op,
            created_ids: (!created_ids.is_empty()).then_some(created_ids),
            changed_ids: Some(changed_ids),
            summary: Some(summary),
            diagnostics: Some(crate::edit_outcome::diagnostics()),
        };
        self.receipts.insert(
            request.operation_id.clone(),
            Recorded {
                request,
                receipt: receipt.clone(),
            },
        );
        Ok(receipt)
    }
    pub fn snapshot(&self) -> Result<Vec<u8>, OperationError> {
        serde_json::to_vec(self).map_err(|e| OperationError::InvalidSnapshot(e.to_string()))
    }
    pub fn restore(bytes: &[u8]) -> Result<Self, OperationError> {
        let mut session: Self = serde_json::from_slice(bytes)
            .or_else(|error| {
                let Some(upgraded) = crate::recovery_relink::upgrade(bytes) else {
                    return Err(error);
                };
                serde_json::from_slice(&upgraded)
            })
            .map_err(|e| OperationError::InvalidSnapshot(e.to_string()))?;
        session
            .history
            .validate_restored()
            .map_err(|e| OperationError::InvalidSnapshot(e.to_string()))?;
        for (id, record) in &session.receipts {
            if id != &record.request.operation_id
                || id != &record.receipt.operation_id
                || record.receipt.revision > session.history.revision()
            {
                return Err(OperationError::InvalidSnapshot("操作記録が不正です".into()));
            }
        }
        Ok(session)
    }
}
