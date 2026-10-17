use sungeki_project::*;

fn entry(written: &str, spoken: &str) -> PronunciationEntry {
    PronunciationEntry {
        written: written.into(),
        spoken: spoken.into(),
    }
}

#[test]
fn project_validation_rejects_ambiguous_or_empty_dictionary() {
    let mut project = Project::empty("00000000-0000-4000-8000-000000000001".into(), "辞書".into());
    project.pronunciation_dictionary = vec![entry("猫", "ねこ"), entry("猫", "キャット")];
    assert!(validate(&project).unwrap_err().0.contains("重複"));
    for invalid in [
        entry(" ", "読む"),
        entry("語", "\n"),
        entry("語\0", "ご"),
        entry("語", "ご\0"),
    ] {
        project.pronunciation_dictionary = vec![invalid];
        assert!(validate(&project).is_err());
    }
    project.pronunciation_dictionary = vec![entry("猫", "ねこ"), entry("猫舌", "ねこじた")];
    assert!(validate(&project).is_ok());
}

#[test]
fn dictionary_limits_cover_count_terms_and_total_bytes() {
    let max_terms: Vec<_> = (0..MAX_DICTIONARY_ENTRIES)
        .map(|i| entry(&format!("語{i}"), "ご"))
        .collect();
    assert!(validate_pronunciation_dictionary(&max_terms).is_ok());
    let mut too_many = max_terms;
    too_many.push(entry("余分", "よぶん"));
    assert!(validate_pronunciation_dictionary(&too_many).is_err());
    assert!(
        validate_pronunciation_dictionary(&[entry(
            &"a".repeat(MAX_WRITTEN_BYTES),
            &"b".repeat(MAX_READING_BYTES)
        )])
        .is_ok()
    );
    assert!(
        validate_pronunciation_dictionary(&[entry(&"a".repeat(MAX_WRITTEN_BYTES + 1), "b")])
            .is_err()
    );
    assert!(
        validate_pronunciation_dictionary(&[entry("a", &"b".repeat(MAX_READING_BYTES + 1))])
            .is_err()
    );
    let large: Vec<_> = (0..100)
        .map(|i| entry(&i.to_string(), &"a".repeat(1024)))
        .collect();
    assert!(
        validate_pronunciation_dictionary(&large)
            .unwrap_err()
            .0
            .contains("64KiB")
    );
}

#[test]
fn dictionary_remains_in_saved_project_and_preserves_exact_strings() {
    let mut project = Project::empty(
        "00000000-0000-4000-8000-000000000001".into(),
        "辞書保存".into(),
    );
    project.pronunciation_dictionary = vec![
        entry("固有名詞", "こゆうめいし"),
        entry("ABC", "エー ビー シー"),
    ];
    let decoded: Project = serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
    assert_eq!(decoded, project);
    assert!(validate(&decoded).is_ok());
}
