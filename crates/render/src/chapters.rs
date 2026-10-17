//! MP4 navigation from the frozen timeline. Chapter bounds are milliseconds,
//! matching the QuickTime chapter track, without changing media timestamps.
use sungeki_project::{ClipContent, Project};

#[derive(Debug, PartialEq)]
pub(crate) struct Chapter {
    pub start_ms: u64,
    pub end_ms: u64,
    pub title: String,
}

pub(crate) fn plan(project: &Project, end_ms: u64) -> Vec<Chapter> {
    // RenderPlan has validated the project. A chapter must seek to a frame
    // showing its content, not the preceding frame when its start is off-grid.
    let start_ms = |time| {
        let fps = &project.canvas.fps;
        let frame =
            sungeki_scene::first_frame_at_or_after(fps, time).expect("validated render timeline");
        if frame == 0 {
            return 0;
        }
        // Seek implementations can round an exact boundary down (e.g. 9.2s
        // to frame 275 at 30fps). Place the marker strictly inside the frame.
        ((u128::from(frame) * u128::from(fps.denominator) * 1000) / u128::from(fps.numerator))
            as u64
            + 1
    };
    let mut entries: Vec<_> = if project.scenes.is_empty() {
        project
            .clips
            .iter()
            .filter_map(|clip| {
                let ClipContent::Speech { subtitle_text, .. } = &clip.content else {
                    return None;
                };
                let track = project.tracks.iter().find(|t| t.id == clip.track_id)?;
                (track.visible || !track.muted)
                    .then_some((start_ms(clip.start_us), title(subtitle_text)))
            })
            .collect()
    } else {
        project
            .scenes
            .iter()
            .map(|scene| (start_ms(scene.start_us), title(&scene.name)))
            .collect()
    };
    entries.retain(|(start, _)| *start < end_ms);
    entries.sort_by_key(|(start, _)| *start);
    // Simultaneous dialogue and boundaries in the same frame cannot form separate
    // MP4 chapters. Keep the first item in saved order deterministically.
    entries.dedup_by_key(|(start, _)| *start);
    if entries.first().is_some_and(|(start, _)| *start > 0) {
        entries.insert(0, (0, title(&project.title)));
    }
    entries
        .iter()
        .enumerate()
        .map(|(i, (start, title))| Chapter {
            start_ms: *start,
            end_ms: entries.get(i + 1).map_or(end_ms, |(start, _)| *start),
            title: title.clone(),
        })
        .collect()
}

fn title(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let title: String = normalized
        .chars()
        .filter(|c| !c.is_control())
        .take(60)
        .collect();
    if title.is_empty() {
        "章".into()
    } else {
        title
    }
}

pub(crate) fn metadata(chapters: &[Chapter]) -> String {
    let mut output = ";FFMETADATA1\n".to_owned();
    for chapter in chapters {
        output.push_str(&format!(
            "[CHAPTER]\nTIMEBASE=1/1000\nSTART={}\nEND={}\n",
            chapter.start_ms, chapter.end_ms,
        ));
    }
    output
}

#[cfg(unix)]
pub(crate) fn verify(
    chapters: &[Chapter],
    probe: &serde_json::Value,
) -> Result<(), crate::RenderError> {
    let actual = probe["chapters"]
        .as_array()
        .ok_or_else(|| crate::plan::error("出力の章情報を取得できません"))?;
    if actual.len() != chapters.len() {
        return Err(crate::plan::error("出力の章数が計画と一致しません"));
    }
    for (actual, expected) in actual.iter().zip(chapters) {
        let matches_time = |key: &str, ms: u64| {
            actual[key]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .is_some_and(|s| s.is_finite() && (s * 1000. - ms as f64).abs() < 0.01)
        };
        if actual["tags"]["title"].as_str() != Some(expected.title.as_str())
            || !matches_time("start_time", expected.start_ms)
            || !matches_time("end_time", expected.end_ms)
        {
            return Err(crate::plan::error("出力の章名・区間が計画と一致しません"));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "chapters_tests.rs"]
mod tests;
