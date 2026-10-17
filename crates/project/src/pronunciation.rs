use crate::{PronunciationEntry, ValidationError};
use std::collections::HashSet;

pub const MAX_DICTIONARY_ENTRIES: usize = 1024;
pub const MAX_DICTIONARY_BYTES: usize = 64 * 1024;
pub const MAX_WRITTEN_BYTES: usize = 256;
pub const MAX_READING_BYTES: usize = 1024;

pub fn validate_pronunciation_dictionary(
    entries: &[PronunciationEntry],
) -> Result<(), ValidationError> {
    if entries.len() > MAX_DICTIONARY_ENTRIES {
        return Err(ValidationError::new("発音辞書は1024項目までです".into()));
    }
    let mut written = HashSet::new();
    let mut bytes = 0;
    for entry in entries {
        if entry.written.trim().is_empty() || entry.spoken.trim().is_empty() {
            return Err(ValidationError::new(
                "辞書の表記と読みは空にできません".into(),
            ));
        }
        if entry.written.len() > MAX_WRITTEN_BYTES || entry.spoken.len() > MAX_READING_BYTES {
            return Err(ValidationError::new(
                "辞書の表記は256バイト、読みは1024バイトまでです".into(),
            ));
        }
        if entry.written.contains('\0') || entry.spoken.contains('\0') {
            return Err(ValidationError::new(
                "発音辞書にNUL文字は使用できません".into(),
            ));
        }
        if !written.insert(&entry.written) {
            return Err(ValidationError::new(
                "発音辞書の表記が重複しています".into(),
            ));
        }
        bytes += entry.written.len() + entry.spoken.len();
        if bytes > MAX_DICTIONARY_BYTES {
            return Err(ValidationError::new("発音辞書は合計64KiBまでです".into()));
        }
    }
    Ok(())
}
