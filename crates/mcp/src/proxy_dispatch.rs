use crate::dispatch::parse;
use rmcp::ErrorData;
use serde_json::Value;
use sungeki_protocol::*;
pub(super) fn request(name: &str, args: Value) -> Result<Option<SessionRequest>, ErrorData> {
    let request = match name {
        "remove_proxy_cache" => ProxyRequest::Remove { args: parse(args)? },
        "generate_proxies" => ProxyRequest::Start { args: parse(args)? },
        "get_proxy_job" => {
            let args: GetJobArgs = parse(args)?;
            ProxyRequest::Get {
                job_id: args.job_id,
                wait_ms: args.wait_ms.unwrap_or(0),
            }
        }
        "cancel_proxy_job" => ProxyRequest::Cancel {
            job_id: parse::<CancelJobArgs>(args)?.job_id,
        },
        "list_proxy_jobs" => {
            let args: ListJobsArgs = parse(args)?;
            ProxyRequest::List {
                session_id: args.session_id,
                after: args.after,
                limit: args.limit.unwrap_or(20),
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(SessionRequest::Proxy { request }))
}
