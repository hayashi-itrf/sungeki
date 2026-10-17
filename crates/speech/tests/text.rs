use sungeki_project::PronunciationEntry;
use sungeki_speech::*;

fn entry(written: &str, spoken: &str) -> PronunciationEntry {
    PronunciationEntry {
        written: written.into(),
        spoken: spoken.into(),
    }
}

#[test]
fn explicit_reading_bypasses_dictionary_and_preserves_subtitle() {
    let subtitle = "羅星詩杏の字幕";
    let dictionary = vec![
        entry("羅星詩杏", "らすたしあん"),
        entry("らすたしあん", "別の読み"),
    ];
    let result = resolve_text(subtitle, Some("らすたしあん\nです。"), &dictionary).unwrap();
    assert_eq!(result.text, "らすたしあん\nです。");
    assert_eq!(result.source, TextSource::ExplicitReading);
    assert!(result.applied.is_empty());
    assert_eq!(subtitle, "羅星詩杏の字幕");
    assert_eq!(
        resolve_text(subtitle, Some("  \n"), &dictionary),
        Err(TextError::EmptyText)
    );
}

#[test]
fn longest_match_is_order_independent_and_replacement_is_never_rescanned() {
    let mut dictionary = vec![
        entry("VITS2", "ヴィッツツー"),
        entry("Style-Bert-VITS2", "VITS2"),
        entry("羅星", "短い読み"),
        entry("羅星詩杏", "らすたしあん"),
    ];
    let input = "Style-Bert-VITS2、VITS2、羅星詩杏。羅星詩杏";
    let result = resolve_text(input, None, &dictionary).unwrap();
    assert_eq!(
        result.text,
        "VITS2、ヴィッツツー、らすたしあん。らすたしあん"
    );
    assert_eq!(
        result.applied,
        vec![
            AppliedPronunciation {
                dictionary_index: 0,
                occurrences: 1
            },
            AppliedPronunciation {
                dictionary_index: 1,
                occurrences: 1
            },
            AppliedPronunciation {
                dictionary_index: 3,
                occurrences: 2
            }
        ]
    );
    dictionary.reverse();
    assert_eq!(
        resolve_text(input, None, &dictionary).unwrap().text,
        result.text
    );
}

#[test]
fn unicode_literal_matching_preserves_unmatched_text_without_normalization() {
    let dictionary = vec![
        entry("猫🐈", "ねこ"),
        entry("a.b", "リテラル"),
        entry("ABC", "大文字"),
    ];
    let result = resolve_text(" 猫🐈\nか\u{3099}が a.b abc ABC 🦊\t", None, &dictionary).unwrap();
    assert_eq!(result.text, " ねこ\nか\u{3099}が リテラル abc 大文字 🦊\t");
    assert_eq!(
        resolve_text("あいうえお", None, &[]).unwrap().text,
        "あいうえお"
    );
    assert_eq!(
        resolve_text("ababa", None, &[entry("aba", "x"), entry("ba", "y")])
            .unwrap()
            .text,
        "xy"
    );
}

#[test]
fn limits_reject_expansion_without_truncating_or_returning_partial_success() {
    let full = "a".repeat(MAX_SPEECH_INPUT_BYTES);
    assert_eq!(
        resolve_text(&full, None, &[]).unwrap().text.len(),
        MAX_SPEECH_INPUT_BYTES
    );
    assert_eq!(
        resolve_text(&(full.clone() + "a"), None, &[]),
        Err(TextError::InputTooLarge)
    );
    let expanded = resolve_text(&full, None, &[entry("a", "bbbb")]).unwrap();
    assert_eq!(expanded.text.len(), MAX_SPEECH_OUTPUT_BYTES);
    assert_eq!(expanded.applied[0].occurrences, MAX_SPEECH_INPUT_BYTES);
    assert_eq!(
        resolve_text(&full, None, &[entry("a", "bbbbb")]),
        Err(TextError::OutputTooLarge)
    );
    assert_eq!(resolve_text("", None, &[]), Err(TextError::EmptyText));
    assert_eq!(resolve_text("a\0b", None, &[]), Err(TextError::InvalidNul));
}

#[test]
fn ambiguous_dictionary_is_not_silently_accepted() {
    assert!(matches!(
        resolve_text("a", None, &[entry("a", "b"), entry("a", "c")]),
        Err(TextError::InvalidDictionary(_))
    ));
}
