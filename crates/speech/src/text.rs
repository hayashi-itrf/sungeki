use std::collections::BTreeMap;
use sungeki_project::{PronunciationEntry, ValidationError, validate_pronunciation_dictionary};

pub const MAX_SPEECH_INPUT_BYTES: usize = 64 * 1024;
pub const MAX_SPEECH_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextSource {
    Subtitle,
    ExplicitReading,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedPronunciation {
    pub dictionary_index: usize,
    pub occurrences: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedText {
    pub text: String,
    pub source: TextSource,
    pub applied: Vec<AppliedPronunciation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextError {
    InvalidDictionary(ValidationError),
    EmptyText,
    InvalidNul,
    InputTooLarge,
    OutputTooLarge,
}
impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDictionary(error) => error.fmt(f),
            Self::EmptyText => f.write_str("発音する文字列は空にできません"),
            Self::InvalidNul => f.write_str("発音する文字列にNUL文字は使用できません"),
            Self::InputTooLarge => f.write_str("合成前の文字列は64KiBまでです"),
            Self::OutputTooLarge => f.write_str("辞書適用後の文字列は256KiBまでです"),
        }
    }
}
impl std::error::Error for TextError {}

#[derive(Default)]
struct Node {
    next: BTreeMap<u8, usize>,
    entry: Option<usize>,
}

/// Left-to-right longest literal match against the original input. Replacement
/// bytes are never scanned again. Case and Unicode normalization are preserved.
pub fn resolve_text(
    subtitle: &str,
    spoken: Option<&str>,
    dictionary: &[PronunciationEntry],
) -> Result<ResolvedText, TextError> {
    validate_pronunciation_dictionary(dictionary).map_err(TextError::InvalidDictionary)?;
    let input = spoken.unwrap_or(subtitle);
    if input.len() > MAX_SPEECH_INPUT_BYTES {
        return Err(TextError::InputTooLarge);
    }
    if input.trim().is_empty() {
        return Err(TextError::EmptyText);
    }
    if input.contains('\0') {
        return Err(TextError::InvalidNul);
    }
    if spoken.is_some() {
        return Ok(ResolvedText {
            text: input.into(),
            source: TextSource::ExplicitReading,
            applied: vec![],
        });
    }
    // Flat indices avoid recursive tree destruction for long dictionary terms.
    let mut nodes = vec![Node::default()];
    for (index, entry) in dictionary.iter().enumerate() {
        let mut node = 0;
        for byte in entry.written.bytes() {
            node = if let Some(next) = nodes[node].next.get(&byte) {
                *next
            } else {
                let next = nodes.len();
                nodes.push(Node::default());
                nodes[node].next.insert(byte, next);
                next
            };
        }
        nodes[node].entry = Some(index);
    }
    let mut text = String::new();
    let mut counts = BTreeMap::<usize, usize>::new();
    let mut start = 0;
    while start < input.len() {
        let mut node = 0;
        let mut matched = None;
        for (offset, byte) in input.as_bytes()[start..].iter().enumerate() {
            let Some(next) = nodes[node].next.get(byte) else {
                break;
            };
            node = *next;
            if let Some(entry) = nodes[node].entry {
                matched = Some((entry, start + offset + 1));
            }
        }
        let (replacement, end) = if let Some((index, end)) = matched {
            *counts.entry(index).or_default() += 1;
            (dictionary[index].spoken.as_str(), end)
        } else {
            let end = start
                + input[start..]
                    .chars()
                    .next()
                    .expect("nonempty remainder")
                    .len_utf8();
            (&input[start..end], end)
        };
        if text.len() + replacement.len() > MAX_SPEECH_OUTPUT_BYTES {
            return Err(TextError::OutputTooLarge);
        }
        text.push_str(replacement);
        start = end;
    }
    Ok(ResolvedText {
        text,
        source: TextSource::Subtitle,
        applied: counts
            .into_iter()
            .map(|(dictionary_index, occurrences)| AppliedPronunciation {
                dictionary_index,
                occurrences,
            })
            .collect(),
    })
}
