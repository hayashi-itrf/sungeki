use crate::{Issue, Severity};
use sungeki_project::Project;
use sungeki_render::TextObservation;
pub(crate) fn append(
    project: &Project,
    observation: &TextObservation,
    percent: u8,
    issues: &mut Vec<Issue>,
) {
    if issues.len() >= 1000 {
        if !issues.iter().any(|i| i.code == "TEXT_DIAGNOSTICS_LIMIT") {
            issues.push(Issue {
                severity: Severity::Warning,
                code: "TEXT_DIAGNOSTICS_LIMIT".into(),
                field: "/clips".into(),
                clip_id: None,
                scene_id: None,
                asset_id: None,
                start_us: None,
                end_us: None,
                message: "文字の詳細が上限に達したため、安全余白の指摘を省略しています。".into(),
            });
        }
        return;
    }
    let Some((index, clip)) = project
        .clips
        .iter()
        .enumerate()
        .find(|(_, c)| c.id == observation.clip_id)
    else {
        return;
    };
    // One warning per interval, even when several lines cross different edges.
    let mut bounds = observation.margin_issues[0].bounds;
    for issue in &observation.margin_issues[1..] {
        bounds[0] = bounds[0].min(issue.bounds[0]);
        bounds[1] = bounds[1].min(issue.bounds[1]);
        bounds[2] = bounds[2].max(issue.bounds[2]);
        bounds[3] = bounds[3].max(issue.bounds[3]);
    }
    issues.push(Issue { severity: Severity::Warning, code: "TEXT_SAFE_MARGIN".into(), field: format!("/clips/{index}/transform"),
        clip_id: Some(clip.id.clone()), scene_id: clip.scene_id.clone(), asset_id: None,
        start_us: project.canvas.fps.frame_time(observation.frame_index), end_us: Some(observation.end_us),
        message: format!("文字が画面端から上下左右{percent}%の安全余白に入っています。意図した配置か確認してください。描画範囲: {bounds:?}") });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn warning_combines_lines_preserves_interval_and_obeys_diagnostic_limit() {
        let p: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        let observation = TextObservation {
            clip_id: p.clips[0].id.clone(),
            frame_index: 3,
            end_us: sungeki_project::TimeUs(1_000_000),
            error: None,
            margin_issues: vec![
                sungeki_render::TextLayoutIssue {
                    code: "TEXT_OVERFLOW".into(),
                    clip_id: p.clips[0].id.clone(),
                    bounds: [0., 10., 20., 30.],
                    outside_canvas: true,
                },
                sungeki_render::TextLayoutIssue {
                    code: "TEXT_OVERFLOW".into(),
                    clip_id: p.clips[0].id.clone(),
                    bounds: [1., 40., 25., 60.],
                    outside_canvas: true,
                },
            ],
        };
        let mut issues = vec![];
        append(&p, &observation, 5, &mut issues);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(issues[0].code, "TEXT_SAFE_MARGIN");
        assert_eq!(issues[0].start_us, Some(sungeki_project::TimeUs(100_000)));
        assert_eq!(issues[0].end_us, Some(observation.end_us));
        assert_eq!(issues[0].scene_id, p.clips[0].scene_id);
        assert!(issues[0].message.contains("[0.0, 10.0, 25.0, 60.0]"));
        issues.resize(1000, issues[0].clone());
        append(&p, &observation, 5, &mut issues);
        append(&p, &observation, 5, &mut issues);
        assert_eq!(issues.len(), 1001);
        assert_eq!(issues[1000].code, "TEXT_DIAGNOSTICS_LIMIT");
    }
}
