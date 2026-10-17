use sungeki_editor::*;

fn roles() -> Vec<String> {
    vec!["left".into(), "right".into()]
}

#[test]
fn keeps_order_unicode_literal_punctuation_and_separate_pronunciation() {
    let source = "\u{feff}## 導入\r\n- left: **Codex**: https://example.test\r\n  > コーデックス\r\n\r\n- right: 日本語です。\r\n## 結び\r\n- left: おしまい。";
    let scenes = parse_markdown_script(source, &roles()).unwrap();
    assert_eq!(scenes.len(), 2);
    assert_eq!(scenes[0].name, "導入");
    assert_eq!(
        scenes[0].lines[0].subtitle_text,
        "**Codex**: https://example.test"
    );
    assert_eq!(
        scenes[0].lines[0].spoken_text.as_deref(),
        Some("コーデックス")
    );
    assert_eq!(scenes[0].lines[1].role, "right");
    assert_eq!(scenes[1].lines[0].spoken_text, None);
}

#[test]
fn reports_source_line_and_rejects_partial_or_ambiguous_documents() {
    for (source, line) in [
        ("- left: 本文", 1),
        ("## 導入\n\n- absent: 本文", 3),
        ("## 導入\n- left:   ", 2),
        ("## 導入\n- left 本文", 2),
        ("## 導入\n### 推測しない", 2),
        ("## 導入\n## 結び\n- left: 本文", 1),
        ("## 導入\n- left: 本文\n## 結び", 3),
        ("## 導入\n  > 読み", 2),
        ("## 導入\n- left: 本文\n\n  > 読み", 4),
        ("## 導入\n- left: 本文\n  > 読み\n  > 二重指定", 4),
        ("## 導入\n- left: 本文\n  >   ", 3),
        ("## 導入\n- left: 本\0文", 2),
        ("## 導入\n- left: 本\r文", 2),
    ] {
        let err = parse_markdown_script(source, &roles()).unwrap_err();
        assert_eq!(err.line, line, "{source}");
        assert!(err.to_string().starts_with(&format!("{line}行目:")));
    }
    assert!(parse_markdown_script("", &roles()).is_err());
    assert!(
        parse_markdown_script("## 導入\n- left: 本文", &["left".into(), "left".into()]).is_err()
    );
}

#[test]
fn bounds_count_and_input_without_truncation() {
    let source = format!("## 導入\n{}", "- left: 本文\n".repeat(200));
    assert_eq!(
        parse_markdown_script(&source, &roles()).unwrap()[0]
            .lines
            .len(),
        200
    );
    assert_eq!(
        parse_markdown_script(&(source + "- right: 超過"), &roles())
            .unwrap_err()
            .line,
        202
    );
    let prefix = "## 導入\n- left: 本文\n";
    let source = format!("{prefix}{}", " ".repeat(100_000 - prefix.chars().count()));
    assert!(parse_markdown_script(&source, &roles()).is_ok());
    assert_eq!(
        parse_markdown_script(&(source + " "), &roles())
            .unwrap_err()
            .line,
        3
    );
    for (prefix, line) in [("## 導入\n- left: ", 2), ("## 導入\n- left: 本文\n  > ", 3)] {
        let valid = format!("{prefix}{}", "a".repeat(65_536));
        assert!(parse_markdown_script(&valid, &roles()).is_ok());
        assert_eq!(
            parse_markdown_script(&(valid + "a"), &roles())
                .unwrap_err()
                .line,
            line
        );
    }
}

#[path = "common/build_recipe.rs"]
mod setup;

#[test]
fn normalized_input_uses_the_existing_recipe_and_atomic_editor_path() {
    let (project, mut recipe, ids) = setup::fixture();
    let roles: Vec<_> = recipe.roles.iter().map(|r| r.role.clone()).collect();
    let source = format!(
        "## 第一章\n- {}: はじめ。\n  > はじめ\n- {}: 次です。\n## 第二章\n- {}: 続き。\n- {}: おしまい。",
        roles[0], roles[1], roles[0], roles[1]
    );
    let parsed = parse_markdown_script(&source, &roles).unwrap();
    for (scene, text) in recipe.scenes.iter_mut().zip(parsed) {
        scene.name = text.name;
        scene.lines = text.lines;
    }
    let commands = plan_build_script(&project, &recipe, &ids).unwrap();
    let mut session = EditSession::new(project.clone()).unwrap();
    session
        .execute(EditRequest {
            operation_id: setup::id(100),
            expected_revision: 0,
            action: EditAction::Apply { commands },
        })
        .unwrap();
    assert_eq!(session.history().project().clips.len(), 4);
    assert_eq!(session.history().project().scenes[1].name, "第二章");
    session
        .execute(EditRequest {
            operation_id: setup::id(101),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: setup::id(100),
            },
        })
        .unwrap();
    assert_eq!(session.history().project(), &project);
}
