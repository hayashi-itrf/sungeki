//! Compact net changes, without serializing whole projects or running media I/O.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use sungeki_project::{Check, CheckKind, CheckStatus, Diagnostics, Project};
use ts_rs::TS;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct EditSummary {
    pub added: u32,
    pub removed: u32,
    pub modified: u32,
    pub reordered_collections: Vec<String>,
    pub project_fields: Vec<String>,
}

pub(crate) fn outcome(
    before: &Project,
    after: &Project,
) -> (BTreeMap<String, Vec<String>>, EditSummary) {
    let mut ids = BTreeMap::new();
    let mut summary = EditSummary::default();
    macro_rules! entities {
        ($field:ident, $ordered:expr) => {
            collection(
                stringify!($field),
                &before.$field,
                &after.$field,
                |item| item.id.as_str(),
                $ordered,
                &mut ids,
                &mut summary,
            );
        };
    }
    entities!(tracks, true);
    entities!(clips, true);
    entities!(scenes, false);
    entities!(groups, false);
    entities!(characters, false);
    entities!(assets, false);
    macro_rules! field {
        ($field:ident) => {
            if before.$field != after.$field {
                summary.project_fields.push(stringify!($field).into());
            }
        };
    }
    field!(format);
    field!(schema_version);
    field!(id);
    field!(title);
    field!(canvas);
    field!(audio_mix);
    field!(pronunciation_dictionary);
    if !summary.project_fields.is_empty() {
        ids.insert(
            "project".into(),
            BTreeSet::from([before.id.clone(), after.id.clone()])
                .into_iter()
                .collect(),
        );
    }
    (ids, summary)
}

fn collection<T: PartialEq>(
    name: &str,
    before: &[T],
    after: &[T],
    id: impl Fn(&T) -> &str,
    ordered: bool,
    output: &mut BTreeMap<String, Vec<String>>,
    summary: &mut EditSummary,
) {
    let a: BTreeMap<_, _> = before.iter().map(|item| (id(item), item)).collect();
    let b: BTreeMap<_, _> = after.iter().map(|item| (id(item), item)).collect();
    let mut changed = BTreeSet::new();
    for key in a.keys().chain(b.keys()).copied().collect::<BTreeSet<_>>() {
        match (a.get(key), b.get(key)) {
            (None, Some(_)) => summary.added += 1,
            (Some(_), None) => summary.removed += 1,
            (Some(first), Some(second)) if first != second => summary.modified += 1,
            _ => continue,
        }
        changed.insert(key.to_owned());
    }
    if ordered {
        // Insert/delete alone does not change the relative order of surviving entities.
        let first: Vec<_> = before
            .iter()
            .map(&id)
            .filter(|key| b.contains_key(key))
            .collect();
        let second: Vec<_> = after
            .iter()
            .map(&id)
            .filter(|key| a.contains_key(key))
            .collect();
        if first != second {
            summary.reordered_collections.push(name.into());
            for (left, right) in first.iter().zip(&second) {
                if left != right {
                    changed.insert((*left).to_owned());
                    changed.insert((*right).to_owned());
                }
            }
        }
    }
    if !changed.is_empty() {
        output.insert(name.into(), changed.into_iter().collect());
    }
}

/// Only called after the common editor has validated the resulting project.
/// An empty issue list is not a claim that assets, speech or rendering passed.
pub(crate) fn diagnostics() -> Diagnostics {
    Diagnostics::new(
        [
            CheckKind::Structure,
            CheckKind::AssetIntegrity,
            CheckKind::TextLayout,
            CheckKind::Audio,
            CheckKind::RepresentativeFrames,
            CheckKind::VisualReview,
        ]
        .into_iter()
        .map(|kind| Check {
            kind,
            status: if kind == CheckKind::Structure {
                CheckStatus::Passed
            } else {
                CheckStatus::NotRun
            },
            description: if kind == CheckKind::Structure {
                "共通編集処理で確定状態の構造を検証済み"
            } else {
                "編集結果では未検査。確認レポートで別途検査してください"
            }
            .into(),
        })
        .collect(),
        Vec::new(),
    )
}
