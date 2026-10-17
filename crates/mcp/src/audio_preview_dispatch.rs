#[path = "proxy_dispatch.rs"]
mod proxy;
use crate::dispatch::parse;
use rmcp::ErrorData;
use serde_json::Value;
use sungeki_protocol::*;
pub fn request(name: &str, args: Value) -> Result<Option<SessionRequest>, ErrorData> {
    if let Some(request) = proxy::request(name, args.clone())? {
        return Ok(Some(request));
    }
    Ok(Some(match name {
        "start_audio_preview_job" => SessionRequest::AudioPreview {
            request: AudioPreviewRequest::Start { args: parse(args)? },
        },
        "get_audio_preview_job" => {
            let args: GetJobArgs = parse(args)?;
            SessionRequest::AudioPreview {
                request: AudioPreviewRequest::Get {
                    job_id: args.job_id,
                    wait_ms: args.wait_ms.unwrap_or(0),
                },
            }
        }
        "cancel_audio_preview_job" => SessionRequest::AudioPreview {
            request: AudioPreviewRequest::Cancel {
                job_id: parse::<CancelJobArgs>(args)?.job_id,
            },
        },
        "list_audio_preview_jobs" => {
            let args: ListJobsArgs = parse(args)?;
            SessionRequest::AudioPreview {
                request: AudioPreviewRequest::List {
                    session_id: args.session_id,
                    after: args.after,
                    limit: args.limit.unwrap_or(20),
                },
            }
        }
        "read_audio_preview" => SessionRequest::AudioPreview {
            request: AudioPreviewRequest::Read { args: parse(args)? },
        },
        _ => return Ok(None),
    }))
}
