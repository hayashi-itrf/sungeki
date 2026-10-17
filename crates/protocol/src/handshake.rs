use crate::{APPLICATION, BUILD_ID, PROTOCOL_MAJOR, PROTOCOL_MINOR};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ClientHello {
    pub application: String,
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub build_id: String,
    pub client_name: String,
    pub capabilities: Vec<String>,
}
impl ClientHello {
    pub fn new(client_name: impl Into<String>) -> Self {
        Self {
            application: APPLICATION.into(),
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            build_id: BUILD_ID.into(),
            client_name: client_name.into(),
            capabilities: vec!["get_capabilities".into()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub editing: bool,
    pub persistence: bool,
    pub speech: bool,
    pub rendering: bool,
    pub vrm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct HostInfo {
    pub application: String,
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub build_id: String,
    pub host_instance_id: String,
    pub capabilities: Capabilities,
}
