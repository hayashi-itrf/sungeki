use crate::dispatch::parse;
use rmcp::ErrorData;
use serde_json::Value;
use sungeki_protocol::*;
pub fn request(name: &str, args: Value) -> Result<Option<SessionRequest>, ErrorData> {
    Ok(Some(match name {
        "collect_project" => SessionRequest::Collection {
            request: CollectionRequest::Start { args: parse(args)? },
        },
        "read_collection_plan" => SessionRequest::Collection {
            request: CollectionRequest::ReadPlan { args: parse(args)? },
        },
        "get_collection_job" => {
            let args: GetJobArgs = parse(args)?;
            SessionRequest::Collection {
                request: CollectionRequest::Get {
                    job_id: args.job_id,
                    wait_ms: args.wait_ms.unwrap_or(0),
                },
            }
        }
        "cancel_collection_job" => SessionRequest::Collection {
            request: CollectionRequest::Cancel {
                job_id: parse::<CancelJobArgs>(args)?.job_id,
            },
        },
        "list_collection_jobs" => {
            let args: ListJobsArgs = parse(args)?;
            SessionRequest::Collection {
                request: CollectionRequest::List {
                    session_id: args.session_id,
                    after: args.after,
                    limit: args.limit.unwrap_or(20),
                },
            }
        }
        _ => return Ok(None),
    }))
}
