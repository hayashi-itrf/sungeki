use crate::{FrameSample, Issue, Severity};
use sungeki_project::Project;
use sungeki_render::RenderError;
pub(crate) fn append(
    project: &Project,
    sample: &FrameSample,
    error: &RenderError,
    result: &mut Vec<Issue>,
) {
    let RenderError::TextLayout(issues) = error else {
        return;
    };
    for issue in issues {
        if result.len() >= 1000 {
            if result.len() == 1000 {
                result.push(Issue { severity: Severity::Warning, code: "TEXT_DIAGNOSTICS_LIMIT".into(), field: "/clips".into(), clip_id: None, scene_id: None, asset_id: None, start_us: None, end_us: None, message: "文字の指摘が1000件に達しました。以降の詳細は省略しています。対象時刻を絞って再確認してください。".into() });
            }
            break;
        }
        let Some((index, clip)) = project
            .clips
            .iter()
            .enumerate()
            .find(|(_, c)| c.id == issue.clip_id)
        else {
            continue;
        };
        result.push(Issue {
            severity: Severity::Error, code: "TEXT_OVERFLOW".into(), field: format!("/clips/{index}/content"),
            clip_id: Some(clip.id.clone()), scene_id: clip.scene_id.clone(), asset_id: None,
            start_us: Some(sample.time_us), end_us: project.canvas.fps.frame_time(sample.frame_index + 1).zip(project.duration()).map(|(end, duration)| end.min(duration)),
            message: format!("{:.3}秒の文字が{}からはみ出しています。文字サイズ・改行・配置を確認してください。描画範囲: {:?}",
                sample.time_us.0 as f64 / 1_000_000.0, if issue.outside_canvas { "画面" } else { "配置枠" }, issue.bounds),
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn targets_are_bounded_to_project_and_diagnostics_have_an_explicit_cap() {
        let mut p: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        p.clips[0].duration_us = sungeki_project::TimeUs(999_999);
        p.scenes[0].duration_us = sungeki_project::TimeUs(999_999);
        let sample = FrameSample {
            frame_index: 29,
            time_us: sungeki_project::TimeUs(966_666),
            reasons: vec![],
        };
        let issue = sungeki_render::TextLayoutIssue {
            code: "TEXT_OVERFLOW".into(),
            clip_id: p.clips[0].id.clone(),
            bounds: [-1., 0., 10., 10.],
            outside_canvas: true,
        };
        let mut found = Vec::new();
        append(
            &p,
            &sample,
            &RenderError::Message("worker failed".into()),
            &mut found,
        );
        assert!(found.is_empty());
        append(
            &p,
            &sample,
            &RenderError::TextLayout(vec![issue; 1100]),
            &mut found,
        );
        assert_eq!(found.len(), 1001);
        assert_eq!(found[0].end_us, Some(sungeki_project::TimeUs(999_999)));
        assert_eq!(found[0].clip_id.as_ref(), Some(&p.clips[0].id));
        assert_eq!(found[1000].code, "TEXT_DIAGNOSTICS_LIMIT");
    }
}
