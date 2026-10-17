//! Strict, bounded Markdown-to-recipe input. No I/O, IDs, or Project mutation.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use sungeki_project::BuildLine;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MarkdownScene {
    pub name: String,
    pub lines: Vec<BuildLine>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MarkdownScriptError {
    /// One-based source line, including blank lines.
    pub line: u32,
    pub message: String,
}
impl std::fmt::Display for MarkdownScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}行目: {}", self.line, self.message)
    }
}
impl std::error::Error for MarkdownScriptError {}
fn error(line: usize, message: &str) -> MarkdownScriptError {
    MarkdownScriptError {
        line: line as u32,
        message: message.into(),
    }
}

/// `## scene`, `- role: subtitle`, optional immediately following `  > spoken`.
/// Blank lines are ignored except that they end the spoken-text continuation.
/// Inline Markdown is literal text. Backgrounds/effects remain explicit recipe input.
pub fn parse_markdown_script(
    source: &str,
    roles: &[String],
) -> Result<Vec<MarkdownScene>, MarkdownScriptError> {
    let known: HashSet<_> = roles.iter().map(String::as_str).collect();
    if roles.is_empty()
        || roles.len() > 200
        || known.len() != roles.len()
        || roles.iter().any(|r| {
            r.trim() != r || r.is_empty() || r.len() > 128 || r.contains([':', '\n', '\r', '\0'])
        })
    {
        return Err(error(
            1,
            "話者の役名は重複しない空でない名前を指定してください（:・改行は使用不可）",
        ));
    }
    let mut scenes: Vec<MarkdownScene> = Vec::new();
    let mut header_line = 1;
    let mut speech_count = 0;
    let mut characters = 0;
    let mut spoken_allowed = false;
    for (index, raw) in source
        .strip_prefix('\u{feff}')
        .unwrap_or(source)
        .split_inclusive('\n')
        .enumerate()
    {
        let line = index + 1;
        characters += raw.chars().count();
        if characters > 100_000 {
            return Err(error(line, "台本は改行を含め100,000文字以内にしてください"));
        }
        let raw = raw.strip_suffix('\n').unwrap_or(raw);
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if raw.contains('\r') {
            return Err(error(line, "改行はLFまたはCRLFを使用してください"));
        }
        if raw.contains('\0') {
            return Err(error(line, "NUL文字は使用できません"));
        }
        if raw.trim().is_empty() {
            spoken_allowed = false;
            continue;
        }
        if let Some(name) = raw.strip_prefix("## ") {
            if scenes.last().is_some_and(|s| s.lines.is_empty()) {
                return Err(error(header_line, "シーンにセリフがありません"));
            }
            let name = name.trim();
            if name.is_empty() || name.len() > 1024 {
                return Err(error(
                    line,
                    "シーン名は空でない1,024バイト以内の名前にしてください",
                ));
            }
            if scenes.len() == 200 {
                return Err(error(line, "シーン数は200件以内にしてください"));
            }
            scenes.push(MarkdownScene {
                name: name.into(),
                lines: Vec::new(),
            });
            header_line = line;
            spoken_allowed = false;
        } else if let Some(body) = raw.strip_prefix("- ") {
            let Some(scene) = scenes.last_mut() else {
                return Err(error(line, "セリフの前に ## シーン名 を指定してください"));
            };
            let Some((role, text)) = body.split_once(':') else {
                return Err(error(
                    line,
                    "セリフは - 役名: 本文 の形式で指定してください",
                ));
            };
            let (role, text) = (role.trim(), text.trim());
            if !known.contains(role) {
                return Err(error(line, "未登録の役名です"));
            }
            if text.is_empty() {
                return Err(error(line, "セリフの本文が空です"));
            }
            if text.len() > 64 * 1024 {
                return Err(error(line, "セリフの本文は65,536バイト以内にしてください"));
            }
            speech_count += 1;
            if speech_count > 200 {
                return Err(error(line, "セリフは全体で200件以内にしてください"));
            }
            scene.lines.push(BuildLine {
                role: role.into(),
                subtitle_text: text.into(),
                spoken_text: None,
                pose: None,
                sprite_pose: None,
            });
            spoken_allowed = true;
        } else if let Some(text) = raw.strip_prefix("  > ") {
            if !spoken_allowed {
                return Err(error(line, "読みはセリフの直後に一度だけ指定してください"));
            }
            if text.trim().is_empty() {
                return Err(error(line, "読みの本文が空です"));
            }
            if text.trim().len() > 64 * 1024 {
                return Err(error(line, "読みの本文は65,536バイト以内にしてください"));
            }
            scenes
                .last_mut()
                .unwrap()
                .lines
                .last_mut()
                .unwrap()
                .spoken_text = Some(text.trim().into());
            spoken_allowed = false;
        } else {
            return Err(error(
                line,
                "未対応の書式です。## シーン名、- 役名: 本文、  > 読み を使用してください",
            ));
        }
    }
    if scenes.is_empty() {
        return Err(error(1, "シーンとセリフを入力してください"));
    }
    if scenes.last().is_some_and(|s| s.lines.is_empty()) {
        return Err(error(header_line, "シーンにセリフがありません"));
    }
    Ok(scenes)
}
