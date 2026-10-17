use crate::dispatch::parse;
use rmcp::ErrorData;
use serde_json::Value;
use sungeki_protocol::*;
pub fn request(name: &str, args: Value) -> Result<Option<SessionRequest>, ErrorData> {
    Ok(Some(match name {
        "read_render_frame" => SessionRequest::RenderJob {
            request: RenderJobRequest::Frame { args: parse(args)? },
        },
        "start_playback_preview" => SessionRequest::RenderJob {
            request: RenderJobRequest::Preview { args: parse(args)? },
        },
        "read_render_output" => SessionRequest::RenderJob {
            request: RenderJobRequest::Read { args: parse(args)? },
        },
        "publish_render_output" => SessionRequest::RenderJob {
            request: RenderJobRequest::Publish { args: parse(args)? },
        },
        "start_render_job" => SessionRequest::RenderJob {
            request: RenderJobRequest::Start { args: parse(args)? },
        },
        "get_render_job" => {
            let args: GetJobArgs = parse(args)?;
            SessionRequest::RenderJob {
                request: RenderJobRequest::Get {
                    job_id: args.job_id,
                    wait_ms: args.wait_ms.unwrap_or(0),
                },
            }
        }
        "cancel_render_job" => SessionRequest::RenderJob {
            request: RenderJobRequest::Cancel {
                job_id: parse::<CancelJobArgs>(args)?.job_id,
            },
        },
        "list_render_jobs" => {
            let args: ListJobsArgs = parse(args)?;
            SessionRequest::RenderJob {
                request: RenderJobRequest::List {
                    session_id: args.session_id,
                    after: args.after,
                    limit: args.limit.unwrap_or(20),
                },
            }
        }
        _ => return Ok(None),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn render_wait_arguments_are_explicit_on_the_common_host_wire() {
        for (args, expected) in [
            (serde_json::json!({"job_id":"j"}), 0),
            (serde_json::json!({"job_id":"j", "wait_ms":30000}), 30000),
        ] {
            let request = request("get_render_job", args).unwrap().unwrap();
            assert_eq!(request.minimum_protocol_minor(), 121);
            assert!(matches!(request, SessionRequest::RenderJob {
                request: RenderJobRequest::Get { job_id, wait_ms }
            } if job_id == "j" && wait_ms == expected));
        }
        assert!(
            request(
                "get_render_job",
                serde_json::json!({"job_id":"j", "wait_ms":-1})
            )
            .is_err()
        );
        assert!(
            request(
                "cancel_render_job",
                serde_json::json!({"job_id":"j", "wait_ms":1})
            )
            .is_err()
        );
    }
}
