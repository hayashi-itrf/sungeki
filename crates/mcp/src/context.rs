use rmcp::ErrorData;
use std::collections::BTreeMap;
use sungeki_protocol::*;

#[derive(Default)]
pub struct Context {
    pub review_attempted: BTreeMap<(String, String), u32>,
    pub attached: Option<String>,
    pub observed: BTreeMap<String, u32>,
    pub attempted: BTreeMap<(String, String), u32>,
}
impl Context {
    pub fn session(&self, explicit: Option<String>) -> Result<String, ErrorData> {
        explicit.or_else(|| self.attached.clone()).ok_or_else(|| {
            ErrorData::invalid_params(
                "先にcreate_projectまたはattach_sessionを実行してください",
                None,
            )
        })
    }
    pub fn revision(&self, id: &str, explicit: Option<u32>) -> Result<u32, ErrorData> {
        explicit
            .or_else(|| self.observed.get(id).copied())
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "expected_revisionを指定するか文書を取得してください",
                    None,
                )
            })
    }
    pub fn operation_revision(
        &self,
        id: &str,
        operation: &str,
        explicit: Option<u32>,
    ) -> Result<u32, ErrorData> {
        self.revision(
            id,
            explicit.or_else(|| self.attempted.get(&(id.into(), operation.into())).copied()),
        )
    }
    pub fn observe(&mut self, response: &SessionResponse, attach: bool) {
        if let SessionResponse::ProjectRead { result } = response {
            self.observed
                .insert(result.session_id.clone(), result.revision);
            return;
        }
        let snapshot = match response {
            SessionResponse::Snapshot { snapshot } | SessionResponse::Edited { snapshot, .. } => {
                snapshot
            }
            _ => return,
        };
        self.observed
            .insert(snapshot.session_id.clone(), snapshot.revision);
        if attach {
            self.attached = Some(snapshot.session_id.clone());
        }
    }
}
