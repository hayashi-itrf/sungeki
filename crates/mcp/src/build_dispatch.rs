use crate::dispatch::parse;
use rmcp::ErrorData;
use serde_json::Value;
use sungeki_protocol::*;
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListBuildArgs {
    pub after: Option<String>,
    pub limit: Option<u32>,
}
pub(crate) fn request(name: &str, args: Value) -> Result<Option<SessionRequest>, ErrorData> {
    let request = match name {
        "build_video" => BuildVideoRequest::Start { args: parse(args)? },
        "reconcile_build_audio" => BuildVideoRequest::ReconcileAudio { args: parse(args)? },
        "read_build_reuse_source" => BuildVideoRequest::ReadReuseSource {
            job_id: parse::<CancelJobArgs>(args)?.job_id,
        },
        "get_build_video" => {
            let args: GetJobArgs = parse(args)?;
            BuildVideoRequest::Get {
                job_id: args.job_id,
                wait_ms: args.wait_ms.unwrap_or(0),
            }
        }
        "cancel_build_video" => BuildVideoRequest::Cancel {
            job_id: parse::<CancelJobArgs>(args)?.job_id,
        },
        "list_build_videos" => {
            let a: ListBuildArgs = parse(args)?;
            BuildVideoRequest::List {
                after: a.after,
                limit: a.limit.unwrap_or(20),
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(SessionRequest::BuildVideo { request }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_wait_is_only_for_get_and_requires_new_host() {
        for wait_ms in [0, 30000] {
            let result = request(
                "get_build_video",
                serde_json::json!({"job_id":"parent", "wait_ms":wait_ms}),
            )
            .unwrap()
            .unwrap();
            assert_eq!(result.minimum_protocol_minor(), 135);
            assert!(
                matches!(result, SessionRequest::BuildVideo { request: BuildVideoRequest::Get { wait_ms: actual, .. } } if actual == wait_ms)
            );
        }
        assert!(
            request(
                "get_build_video",
                serde_json::json!({"job_id":"parent", "wait_ms":-1})
            )
            .is_err()
        );
        assert!(
            request(
                "cancel_build_video",
                serde_json::json!({"job_id":"parent", "wait_ms":1})
            )
            .is_err()
        );
    }
    #[test]
    fn build_tools_route_strictly_and_default_list_limit() {
        assert!(matches!(
            request("list_build_videos", serde_json::json!({})).unwrap(),
            Some(SessionRequest::BuildVideo {
                request: BuildVideoRequest::List {
                    after: None,
                    limit: 20
                }
            })
        ));
        for name in ["get_build_video", "cancel_build_video"] {
            assert!(matches!(
                request(name, serde_json::json!({"job_id":"parent"})).unwrap(),
                Some(SessionRequest::BuildVideo { .. })
            ));
            assert!(request(name, serde_json::json!({"job_id":"parent", "restart":true})).is_err());
        }
        assert!(matches!(
            request(
                "reconcile_build_audio",
                serde_json::json!({"job_id":"p", "child_job_id":"c"})
            )
            .unwrap(),
            Some(SessionRequest::BuildVideo {
                request: BuildVideoRequest::ReconcileAudio { .. }
            })
        ));
        assert!(request("reconcile_build_audio", serde_json::json!({"job_id":"p"})).is_err());
        assert!(request("build_video", serde_json::json!({})).is_err());
        assert!(
            request("unrelated", serde_json::json!({}))
                .unwrap()
                .is_none()
        );
    }
}
