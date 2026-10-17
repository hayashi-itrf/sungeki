use crate::dispatch::parse;
use rmcp::ErrorData;
use serde_json::Value;
use sungeki_protocol::*;
pub fn request(name: &str, args: Value) -> Result<Option<SessionRequest>, ErrorData> {
    Ok(Some(match name {
        "read_frame_preview" => SessionRequest::FramePreview {
            request: FramePreviewRequest::Read { args: parse(args)? },
        },
        "publish_frame_preview" => SessionRequest::FramePreview {
            request: FramePreviewRequest::Publish { args: parse(args)? },
        },
        "preview_frame" => SessionRequest::FramePreview {
            request: FramePreviewRequest::Start { args: parse(args)? },
        },
        "get_frame_preview_job" => SessionRequest::FramePreview {
            request: FramePreviewRequest::Get {
                job_id: parse::<CancelJobArgs>(args)?.job_id,
            },
        },
        "cancel_frame_preview_job" => SessionRequest::FramePreview {
            request: FramePreviewRequest::Cancel {
                job_id: parse::<CancelJobArgs>(args)?.job_id,
            },
        },
        "list_frame_preview_jobs" => {
            let args: ListJobsArgs = parse(args)?;
            SessionRequest::FramePreview {
                request: FramePreviewRequest::List {
                    session_id: args.session_id,
                    after: args.after,
                    limit: args.limit.unwrap_or(20),
                },
            }
        }
        _ => return Ok(None),
    }))
}
