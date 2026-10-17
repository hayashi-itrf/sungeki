use super::ProbeError;
use serde::Deserialize;

pub use sungeki_project::{
    MediaRatio as Rational, MediaStreamClock as StreamClock, VideoAudioStreamInfo as AudioStream,
    VideoMetadata, VideoStreamInfo as VideoStream,
};
#[derive(Deserialize)]
struct Document {
    format: Format,
    streams: Vec<Stream>,
}
#[derive(Deserialize)]
struct Format {
    format_name: String,
}
#[derive(Deserialize)]
struct Stream {
    index: u32,
    codec_type: String,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    sample_aspect_ratio: Option<String>,
    time_base: Option<String>,
    start_pts: Option<i64>,
    duration_ts: Option<i64>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    nb_frames: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    #[serde(default)]
    disposition: Disposition,
    #[serde(default)]
    side_data_list: Vec<SideData>,
    #[serde(default)]
    tags: std::collections::HashMap<String, String>,
}
#[derive(Default, Deserialize)]
struct Disposition {
    #[serde(default)]
    attached_pic: u32,
}
#[derive(Deserialize)]
struct SideData {
    side_data_type: String,
    rotation: Option<f64>,
    displaymatrix: Option<String>,
}
fn invalid() -> ProbeError {
    ProbeError::Invalid("stream metadata")
}
fn ratio(value: Option<&str>, separator: char) -> Result<Option<Rational>, ProbeError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if matches!(value, "N/A" | "0/0" | "0:1" | "0/1") {
        return Ok(None);
    }
    let (n, d) = value.split_once(separator).ok_or_else(invalid)?;
    let numerator = n.parse::<u32>().map_err(|_| invalid())?;
    let denominator = d.parse::<u32>().map_err(|_| invalid())?;
    if numerator == 0 || denominator == 0 {
        return Err(invalid());
    }
    Ok(Some(Rational {
        numerator,
        denominator,
    }))
}
impl Stream {
    fn clock(&self) -> Result<StreamClock, ProbeError> {
        Ok(StreamClock {
            time_base: ratio(self.time_base.as_deref(), '/')?.ok_or_else(invalid)?,
            start_pts: self.start_pts,
            duration_ts: self
                .duration_ts
                .map(|v| u64::try_from(v).ok().filter(|n| *n > 0).ok_or_else(invalid))
                .transpose()?,
        })
    }
    fn codec(&self) -> Result<String, ProbeError> {
        self.codec_name
            .clone()
            .filter(|v| !v.is_empty() && v.len() <= 128)
            .ok_or_else(invalid)
    }
    fn video(&self) -> Result<VideoStream, ProbeError> {
        let width = self
            .width
            .filter(|n| (1..=16384).contains(n))
            .ok_or_else(invalid)?;
        let height = self
            .height
            .filter(|n| (1..=16384).contains(n))
            .ok_or_else(invalid)?;
        if u64::from(width) * u64::from(height) > 100_000_000 {
            return Err(invalid());
        }
        let matrices: Vec<_> = self
            .side_data_list
            .iter()
            .filter(|s| s.side_data_type == "Display Matrix")
            .collect();
        if matrices.len() > 1 {
            return Err(invalid());
        }
        let matrix = matrices.first();
        let rotation_degrees = matrix.and_then(|m| m.rotation);
        if rotation_degrees.is_some_and(|v| !v.is_finite() || v.abs() > 3600.0) {
            return Err(invalid());
        }
        let frame_count = self
            .nb_frames
            .as_deref()
            .filter(|v| *v != "N/A")
            .map(|v| v.parse::<u64>().ok().filter(|n| *n > 0).ok_or_else(invalid))
            .transpose()?;
        Ok(VideoStream {
            index: self.index,
            codec: self.codec()?,
            width,
            height,
            sample_aspect_ratio: ratio(self.sample_aspect_ratio.as_deref(), ':')?,
            clock: self.clock()?,
            average_frame_rate: ratio(self.avg_frame_rate.as_deref(), '/')?,
            nominal_frame_rate: ratio(self.r_frame_rate.as_deref(), '/')?,
            frame_count,
            rotation_degrees,
            display_matrix: matrix.and_then(|m| m.displaymatrix.clone()),
            legacy_rotate_tag: self.tags.get("rotate").cloned(),
        })
    }
    fn audio(&self) -> Result<AudioStream, ProbeError> {
        Ok(AudioStream {
            index: self.index,
            codec: self.codec()?,
            clock: self.clock()?,
            sample_rate: self
                .sample_rate
                .as_deref()
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|n| (1..=768000).contains(n))
                .ok_or_else(invalid)?,
            channels: self
                .channels
                .filter(|n| (1..=64).contains(n))
                .ok_or_else(invalid)?,
        })
    }
}
pub(super) fn parse(bytes: &[u8]) -> Result<VideoMetadata, ProbeError> {
    let document: Document = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if document.streams.len() > 64 {
        return Err(invalid());
    }
    let mut ids = std::collections::HashSet::new();
    if document.streams.iter().any(|s| !ids.insert(s.index)) {
        return Err(invalid());
    }
    let videos: Vec<_> = document
        .streams
        .iter()
        .filter(|s| s.codec_type == "video" && s.disposition.attached_pic == 0)
        .collect();
    if videos.len() != 1 {
        return Err(ProbeError::Invalid(
            "exactly one moving video stream required",
        ));
    }
    let audio = document
        .streams
        .iter()
        .filter(|s| s.codec_type == "audio")
        .map(Stream::audio)
        .collect::<Result<Vec<_>, _>>()?;
    let container = match document.format.format_name.as_str() {
        "mov,mp4,m4a,3gp,3g2,mj2" => sungeki_project::VideoContainer::Mov,
        "matroska,webm" => sungeki_project::VideoContainer::Matroska,
        _ => return Err(ProbeError::Invalid("unsupported container")),
    };
    Ok(VideoMetadata {
        container,
        video: videos[0].video()?,
        audio,
    })
}
#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
