//! Plain subtitle sidecars from the saved timeline, independent of TTS/FFmpeg.
use crate::{RenderError, plan::error};
use sungeki_project::{ClipContent, Project};

#[derive(Debug, Clone, Copy)]
pub enum SubtitleFormat {
    Srt,
    WebVtt,
}

/// Speech captions only by default; title/credit Text clips require opt-in.
/// Uses current clip bounds, not spoken_text or the source WAV duration.
/// Millisecond rounding expands each cue by less than one millisecond per end.
pub fn subtitle_document(
    project: &Project,
    format: SubtitleFormat,
    include_text: bool,
) -> Result<String, RenderError> {
    sungeki_project::validate(project).map_err(error)?;
    let visible: std::collections::HashSet<_> = project
        .tracks
        .iter()
        .filter(|track| track.visible)
        .map(|track| track.id.as_str())
        .collect();
    let mut cues = project
        .clips
        .iter()
        .enumerate()
        .filter(|(_, clip)| visible.contains(clip.track_id.as_str()))
        .filter_map(|(index, clip)| {
            let text = match &clip.content {
                ClipContent::Speech { subtitle_text, .. } => subtitle_text,
                ClipContent::Text { text, .. } if include_text => text,
                _ => return None,
            };
            (!text.trim().is_empty()).then_some((index, clip, text))
        })
        .collect::<Vec<_>>();
    cues.sort_by_key(|(index, clip, _)| (clip.start_us.0, *index));
    let mut output = match format {
        SubtitleFormat::Srt => String::new(),
        SubtitleFormat::WebVtt => "WEBVTT\n\n".to_owned(),
    };
    for (number, (_, clip, text)) in cues.into_iter().enumerate() {
        let start = timestamp(clip.start_us.0 / 1000, format);
        let end = timestamp(
            (clip.start_us.0 + clip.duration_us.0).div_ceil(1000),
            format,
        );
        output.push_str(&format!(
            "{}\n{start} --> {end}\n{}\n\n",
            number + 1,
            cue_text(text)
        ));
    }
    Ok(output)
}
fn timestamp(ms: u64, format: SubtitleFormat) -> String {
    let separator = match format {
        SubtitleFormat::Srt => ',',
        SubtitleFormat::WebVtt => '.',
    };
    format!(
        "{:02}:{:02}:{:02}{separator}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}
fn cue_text(text: &str) -> String {
    // Escape markup-like dialogue. Blank lines must not terminate a cue.
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                "\u{a0}".to_owned()
            } else {
                line.replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "subtitles_tests.rs"]
mod tests;
