//! Range delivery delegates every byte read to the common host's verified artifact API.
use base64::{Engine, engine::general_purpose::STANDARD};
use sungeki_protocol::*;
use tauri::http::{Request, Response};

pub async fn respond(request: Request<Vec<u8>>, video: bool) -> Response<Vec<u8>> {
    match serve(request, video).await {
        Ok(response) => response,
        Err(message) => Response::builder()
            .status(416)
            .header("Cache-Control", "no-store")
            .body(message.into_bytes())
            .expect("constant response"),
    }
}
async fn serve(request: Request<Vec<u8>>, video: bool) -> Result<Response<Vec<u8>>, String> {
    let id = request.uri().path().trim_start_matches('/').to_string();
    if id.len() != 36 || id.bytes().any(|c| !c.is_ascii_hexdigit() && c != b'-') {
        return Err("確認音声のIDが不正です".into());
    }
    let query = if video {
        SessionRequest::RenderJob {
            request: RenderJobRequest::Get {
                job_id: id.clone(),
                wait_ms: 0,
            },
        }
    } else {
        SessionRequest::AudioPreview {
            request: AudioPreviewRequest::Get {
                job_id: id.clone(),
                wait_ms: 0,
            },
        }
    };
    let response = crate::host::session_request(query)
        .await
        .map_err(|e| e.message)?;
    let total = match response {
        SessionResponse::RenderJob { job } if video => {
            job.output.ok_or("動画は未完成です")?.byte_length
        }
        SessionResponse::AudioPreviewJob { job } if !video => {
            job.output.ok_or("音声は未完成です")?.byte_length
        }
        _ => return Err("応答不一致".into()),
    };
    let mime = if video { "video/mp4" } else { "audio/mp4" };
    if request.method() == "HEAD" {
        return Response::builder()
            .header("Content-Type", mime)
            .header("Content-Length", total)
            .header("Accept-Ranges", "bytes")
            .body(Vec::new())
            .map_err(|e| e.to_string());
    }
    if request.method() != "GET" {
        return Err("GETのみ対応".into());
    }
    let range = request
        .headers()
        .get("range")
        .map(|v| v.to_str().map_err(|e| e.to_string()))
        .transpose()?;
    let (start, end) = bounds(range, total)?;
    let query = if video {
        SessionRequest::RenderJob {
            request: RenderJobRequest::Read {
                args: ReadRenderArgs {
                    job_id: id,
                    offset: start,
                    length: (end - start + 1) as u32,
                },
            },
        }
    } else {
        SessionRequest::AudioPreview {
            request: AudioPreviewRequest::Read {
                args: ReadAudioPreviewArgs {
                    job_id: id,
                    offset: start,
                    length: (end - start + 1) as u32,
                },
            },
        }
    };
    let response = crate::host::session_request(query)
        .await
        .map_err(|e| e.message)?;
    let (offset, returned_total, data) = match response {
        SessionResponse::RenderBytes { data } if video => (data.offset, data.total, data.base64),
        SessionResponse::AudioPreviewBytes { data } if !video => {
            (data.offset, data.total, data.base64)
        }
        _ => return Err("応答不一致".into()),
    };
    let bytes = STANDARD.decode(&data).map_err(|e| e.to_string())?;
    if returned_total != total || offset != start || bytes.len() as u64 != end - start + 1 {
        return Err("取得範囲が不一致".into());
    }
    let mut builder = Response::builder()
        .status(if range.is_some() { 206 } else { 200 })
        .header("Content-Type", mime)
        .header("Content-Length", bytes.len())
        .header("Cache-Control", "no-store")
        .header("Accept-Ranges", "bytes");
    if range.is_some() {
        builder = builder.header("Content-Range", format!("bytes {start}-{end}/{total}"));
    }
    builder.body(bytes).map_err(|e| e.to_string())
}
fn bounds(range: Option<&str>, total: u64) -> Result<(u64, u64), String> {
    if total == 0 {
        return Err("空の音声".into());
    }
    let (start, end) = match range {
        None if total <= 262144 => (0, total - 1),
        None => return Err("Range要求が必要です".into()),
        Some(value) => {
            let (a, b) = value
                .strip_prefix("bytes=")
                .and_then(|v| v.split_once('-'))
                .ok_or("不正なRange")?;
            if a.is_empty() {
                let tail = b.parse::<u64>().map_err(|_| "不正なRange")?;
                if tail == 0 {
                    return Err("不正なRange".into());
                }
                (total.saturating_sub(tail), total - 1)
            } else {
                let start = a.parse::<u64>().map_err(|_| "不正なRange")?;
                let end = if b.is_empty() {
                    total - 1
                } else {
                    b.parse::<u64>().map_err(|_| "不正なRange")?.min(total - 1)
                };
                (start, end)
            }
        }
    };
    if start >= total || end < start {
        return Err("範囲外のRange".into());
    }
    Ok((start, end.min(start.saturating_add(262143))))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_ranges() {
        assert_eq!(bounds(Some("bytes=0-1"), 1000000).unwrap(), (0, 1));
        assert_eq!(bounds(Some("bytes=0-"), 1000000).unwrap(), (0, 262143));
        assert_eq!(bounds(Some("bytes=-20"), 100).unwrap(), (80, 99));
        for s in ["bytes=100-", "bytes=5-1", "bytes=0-1,4-5", "bytes=-0"] {
            assert!(bounds(Some(s), 100).is_err());
        }
    }
}
