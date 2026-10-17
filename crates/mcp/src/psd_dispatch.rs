use crate::dispatch::parse;
use rmcp::ErrorData;
use serde_json::Value;
use sungeki_protocol::*;

pub(crate) fn request(name: &str, args: Value) -> Result<Option<SessionRequest>, ErrorData> {
    Ok(match name {
        "preview_video" => Some(SessionRequest::VideoPreview { args: parse(args)? }),
        "preview_psd" => {
            let args: PsdPreviewArgs = parse(args)?;
            Some(SessionRequest::PsdPreview {
                session_id: args.session_id,
                expected_revision: args.expected_revision,
                clip_id: args.clip_id,
                time_us: args.time_us,
            })
        }
        "probe_psd" => Some(SessionRequest::ProbePsd {
            path: parse::<ProbeImageArgs>(args)?.path,
        }),
        _ => None,
    })
}
