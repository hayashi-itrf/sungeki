//! Recorded timing only: no original-file, loudness or listening verification.
use crate::{Check, CheckKind, CheckStatus, Issue, Severity};
use std::collections::HashMap;
use sungeki_project::{AssetProbe, Clip, ClipContent, Project, TimeUs};

pub fn review_audio_timing(project: &Project) -> (Check, Vec<Issue>) {
    let mut check = Check {
        kind: CheckKind::Audio,
        status: CheckStatus::NotRun,
        description: "登録済み音声の尺と発話配置を確認。原本の再検査・音量測定・聴き取りは未実施です。重なりは各発話につき最も長く続く先行発話との1件を表示します。".into(),
    };
    if sungeki_project::validate(project).is_err() {
        check.description =
            "作品の構造に問題があるため発話配置は未検査です。原本・音量・聴き取りも未検査です。"
                .into();
        return (check, vec![]);
    }
    let tracks: HashMap<_, _> = project.tracks.iter().map(|t| (t.id.as_str(), t)).collect();
    let assets: HashMap<_, _> = project.assets.iter().map(|a| (a.id.as_str(), a)).collect();
    let mut issues = Vec::new();
    let mut windows = Vec::new();
    for clip in &project.clips {
        let ClipContent::Speech { audio, .. } = &clip.content else {
            continue;
        };
        if tracks[clip.track_id.as_str()].muted {
            continue;
        }
        let Some(audio) = audio else {
            issues.push(issue(
                clip,
                None,
                Severity::Warning,
                "SPEECH_AUDIO_NOT_GENERATED",
                clip.start_us,
                TimeUs(clip.start_us.0 + clip.duration_us.0),
                "音声が未合成のため、実際の発話尺と重なりを確認できません。".into(),
            ));
            continue;
        };
        let AssetProbe::Audio(probe) = &assets[audio.asset_id.as_str()].probe else {
            continue;
        };
        if clip.duration_us.0 < probe.duration_us.0 {
            issues.push(issue(clip, Some(&audio.asset_id), Severity::Error, "SPEECH_AUDIO_TRUNCATED",
                clip.start_us, TimeUs(clip.start_us.0 + clip.duration_us.0),
                format!("発話区間が登録済み音声より{}マイクロ秒短く、末尾まで収まらないため正式動画を出力できません。区間を延ばしてください。", probe.duration_us.0 - clip.duration_us.0)));
        }
        // Silence after the source ends does not count as a second speaker.
        let end = TimeUs(clip.start_us.0 + clip.duration_us.0.min(probe.duration_us.0));
        windows.push((clip, &audio.asset_id, end));
    }
    windows.sort_by(|a, b| (a.0.start_us, &a.0.id).cmp(&(b.0.start_us, &b.0.id)));
    let mut furthest: Option<(&Clip, TimeUs)> = None;
    for (clip, asset, end) in windows {
        if let Some((prior, prior_end)) = furthest {
            if prior_end.0 > clip.start_us.0 {
                issues.push(issue(
                    clip,
                    Some(asset),
                    Severity::Warning,
                    "SPEECH_OVERLAP",
                    clip.start_us,
                    TimeUs(prior_end.0.min(end.0)),
                    format!(
                        "{}と{}の音声区間が重なっています。意図した同時発話か確認してください。",
                        speech_label(clip),
                        speech_label(prior)
                    ),
                ));
            }
        }
        if furthest.is_none_or(|(_, previous)| end.0 > previous.0) {
            furthest = Some((clip, end));
        }
    }
    if issues.iter().any(|i| i.severity == Severity::Error) {
        check.status = CheckStatus::Failed;
    }
    (check, issues)
}
fn speech_label(clip: &Clip) -> String {
    let ClipContent::Speech { subtitle_text, .. } = &clip.content else {
        return clip.id.clone();
    };
    let mut chars = subtitle_text.trim().chars();
    let excerpt: String = chars
        .by_ref()
        .take(40)
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let suffix = if chars.next().is_some() { "…" } else { "" };
    let ms = clip.start_us.0 / 1_000;
    format!(
        "発話「{excerpt}{suffix}」（開始 {}:{:02}.{:03}）",
        ms / 60_000,
        ms / 1_000 % 60,
        ms % 1_000
    )
}
fn issue(
    clip: &Clip,
    asset: Option<&str>,
    severity: Severity,
    code: &str,
    start: TimeUs,
    end: TimeUs,
    message: String,
) -> Issue {
    Issue {
        severity,
        code: code.into(),
        field: format!("/clips/{}/content/audio", clip.id),
        clip_id: Some(clip.id.clone()),
        scene_id: clip.scene_id.clone(),
        asset_id: asset.map(str::to_owned),
        start_us: Some(start),
        end_us: Some(end),
        message,
    }
}
