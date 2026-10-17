use crate::{
    image_probe::video::{Admission, error},
    render_record::{decode, invalid, record},
    sessions::Sessions,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use sungeki_media::video_probe::{
    ProbeRuntime,
    rendered_frame::{FrameRequest, png},
};
use sungeki_protocol::*;

pub(super) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: RenderFrameArgs,
) -> Result<SessionResponse, ApiError> {
    let (input, output, runtime) = {
        let sessions = shared.lock().map_err(invalid)?;
        let (input, state) = decode(&record(&sessions, &args.job_id)?)?;
        let output = state.output.ok_or_else(|| {
            ApiError::new(ErrorCode::ValidationFailed, "完成した動画がありません")
        })?;
        (input, output, sessions.render_runtime.clone())
    };
    if Path::new(&output.path) != input.directory.join("output.mp4") {
        return Err(invalid("完成動画の記録先が不一致です"));
    }
    let canvas = &input.project.canvas;
    let index = frame_index(
        args.time_us.0,
        output.frame_count,
        canvas.fps.numerator,
        canvas.fps.denominator,
    )?;
    let longest = canvas.width.max(canvas.height);
    if longest == 0 || canvas.width == 0 || canvas.height == 0 {
        return Err(invalid("動画寸法が不正です"));
    }
    let scale = 960.0_f64.min(f64::from(longest)) / f64::from(longest);
    let width = (f64::from(canvas.width) * scale).round().max(1.0) as u32;
    let height = (f64::from(canvas.height) * scale).round().max(1.0) as u32;
    let admission = Admission::acquire(runtime)?;
    let bytes = png(
        &FrameRequest {
            source: Path::new(&output.path),
            sha256: &output.sha256,
            byte_length: output.byte_length,
            frame_index: index,
            frame_count: output.frame_count,
            width,
            height,
        },
        &input.runtime.ffmpeg,
        &ProbeRuntime {
            ffprobe: &input.runtime.ffprobe,
            root: &input.directory,
            timeout: Duration::from_secs(60),
            max_source_bytes: 4 * 1024 * 1024 * 1024,
        },
        &admission.runtime.stopping,
    )
    .map_err(error)?;
    Ok(SessionResponse::RenderFrame {
        image: RenderFrame {
            job_id: args.job_id,
            time_us: args.time_us,
            frame_index: index,
            width,
            height,
            png_base64: STANDARD.encode(bytes),
        },
    })
}

fn frame_index(time: u64, count: u64, numerator: u32, denominator: u32) -> Result<u64, ApiError> {
    if count == 0 || numerator == 0 || denominator == 0 {
        return Err(invalid("出力フレーム情報が不正です"));
    }
    let scaled_time = u128::from(time) * u128::from(numerator);
    let unit = 1_000_000 * u128::from(denominator);
    // MP4 movie headers use a millisecond clock. Native players can report
    // that rounded-up endpoint, slightly beyond the video stream duration.
    // Accept only that final partial millisecond and return the last frame;
    // never shift the playback clock or use the current Project's FPS.
    let end_us = (u128::from(count) * unit).div_ceil(u128::from(numerator) * 1_000) * 1_000;
    if u128::from(time) > end_us {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "動画の範囲外の時刻です",
        ));
    }
    Ok((scaled_time / unit).min(u128::from(count - 1)) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mp4_rounded_endpoint_returns_last_frame_but_rejects_later_times() {
        // Native 115.234 s endpoint vs 3457/30 s video stream.
        for time in [115_233_333, 115_233_334, 115_234_000] {
            assert_eq!(frame_index(time, 3457, 30, 1).unwrap(), 3456);
        }
        assert!(frame_index(115_234_001, 3457, 30, 1).is_err());
        // Rational FPS also rounds only the endpoint, not interior frames.
        assert_eq!(frame_index(33_366, 2, 30_000, 1001).unwrap(), 0);
        assert_eq!(frame_index(33_367, 2, 30_000, 1001).unwrap(), 1);
        assert_eq!(frame_index(67_000, 2, 30_000, 1001).unwrap(), 1);
        assert!(frame_index(67_001, 2, 30_000, 1001).is_err());
    }
    #[test]
    fn rendered_frame_clock_handles_boundaries_and_rational_fps() {
        assert_eq!(frame_index(7_460_000, 447, 30, 1).unwrap(), 223);
        assert_eq!(frame_index(14_900_000, 447, 30, 1).unwrap(), 446);
        assert!(frame_index(14_900_001, 447, 30, 1).is_err());
        assert_eq!(frame_index(1_000_999, 60, 30_000, 1001).unwrap(), 29);
        assert_eq!(frame_index(1_001_000, 60, 30_000, 1001).unwrap(), 30);
        assert!(frame_index(u64::MAX, 447, 30, 1).is_err());
        assert!(frame_index(0, 0, 30, 1).is_err());
        assert!(frame_index(0, 1, 0, 1).is_err());
    }
}
