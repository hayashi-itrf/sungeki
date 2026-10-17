use crate::{
    ApiError, ClientHello, EventBatch, EventCursor, HostInfo, SessionRequest, SessionResponse,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub request_id: u32,
    pub body: RequestBody,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum RequestBody {
    Hello {
        hello: ClientHello,
    },
    GetCapabilities {},
    Lifecycle {
        action: crate::ExitAction,
    },
    Watch {
        after: Option<EventCursor>,
        wait_ms: u32,
    },
    Session {
        request: SessionRequest,
    },
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub request_id: u32,
    pub body: ResponseBody,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum ResponseBody {
    Lifecycle { status: crate::ExitStatus },
    Hello { host: HostInfo },
    Capabilities { host: HostInfo },
    Error { error: ApiError },
    Events { batch: EventBatch },
    Session { result: SessionResponse },
}
