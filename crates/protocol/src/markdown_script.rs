use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ParseMarkdownScriptArgs {
    pub source: String,
    pub roles: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionRequest;
    #[test]
    fn markdown_requires_new_host_and_strict_arguments() {
        let request = SessionRequest::ParseMarkdownScript {
            args: ParseMarkdownScriptArgs {
                source: "## 導入\n- left: 本文".into(),
                roles: vec!["left".into()],
            },
        };
        assert_eq!(request.minimum_protocol_minor(), 144);
        assert!(
            serde_json::from_value::<ParseMarkdownScriptArgs>(serde_json::json!({
                "source": "", "roles": ["left"], "session_id": "must-not-edit"
            }))
            .is_err()
        );
    }
}
