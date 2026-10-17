use crate::dispatch::parse;
use rmcp::ErrorData;
use serde_json::Value;
use sungeki_protocol::*;
pub fn request(name: &str, args: Value) -> Result<Option<SessionRequest>, ErrorData> {
    Ok(Some(match name {
        "probe_vrm" => {
            let args = parse::<ProbeVrmArgs>(args)?;
            SessionRequest::ProbeVrm {
                path: args.path,
                kind: args.kind,
            }
        }
        "probe_video" => SessionRequest::ProbeVideo {
            path: parse::<ProbeVideoArgs>(args)?.path,
        },
        "probe_audio" => SessionRequest::ProbeAudio {
            path: parse::<ProbeAudioArgs>(args)?.path,
        },

        "propose_speech_gain" => SessionRequest::ProposeSpeechGain { args: parse(args)? },
        "start_loudness_job" => SessionRequest::LoudnessJob {
            request: LoudnessJobRequest::Start { args: parse(args)? },
        },
        "get_loudness_job" => {
            let args: GetJobArgs = parse(args)?;
            SessionRequest::LoudnessJob {
                request: LoudnessJobRequest::Get {
                    job_id: args.job_id,
                    wait_ms: args.wait_ms.unwrap_or(0),
                },
            }
        }
        "cancel_loudness_job" => SessionRequest::LoudnessJob {
            request: LoudnessJobRequest::Cancel {
                job_id: parse::<CancelJobArgs>(args)?.job_id,
            },
        },
        "list_loudness_jobs" => {
            let args: ListJobsArgs = parse(args)?;
            SessionRequest::LoudnessJob {
                request: LoudnessJobRequest::List {
                    session_id: args.session_id,
                    after: args.after,
                    limit: args.limit.unwrap_or(20),
                },
            }
        }
        _ => return Ok(None),
    }))
}
