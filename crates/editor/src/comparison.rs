use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use sungeki_project::{Project, ValidationError};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum DifferenceKind {
    Added,
    Removed,
    Changed,
    Reordered,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ValuePreview {
    pub json: String,
    pub truncated: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProjectDifference {
    pub collection: String,
    pub entity_id: Option<String>,
    pub field: String,
    pub kind: DifferenceKind,
    pub before: Option<ValuePreview>,
    pub after: Option<ValuePreview>,
}
fn preview(value: Option<&Value>, limit: usize) -> Option<ValuePreview> {
    value.map(|v| {
        let json = v.to_string();
        ValuePreview {
            truncated: json.chars().count() > limit,
            json: json.chars().take(limit).collect(),
        }
    })
}
fn entry(
    out: &mut Vec<ProjectDifference>,
    collection: &str,
    id: Option<&str>,
    field: &str,
    kind: DifferenceKind,
    before: Option<&Value>,
    after: Option<&Value>,
) {
    // Text edits need a readable review; other fields keep compact summaries.
    let limit = if field == "/content/text" { 4096 } else { 80 };
    out.push(ProjectDifference {
        collection: collection.into(),
        entity_id: id.map(str::to_owned),
        field: field.into(),
        kind,
        before: preview(before, limit),
        after: preview(after, limit),
    });
}
fn fields(
    out: &mut Vec<ProjectDifference>,
    collection: &str,
    id: Option<&str>,
    path: &str,
    before: &Value,
    after: &Value,
) {
    if before == after {
        return;
    }
    if let (Some(a), Some(b)) = (before.as_object(), after.as_object()) {
        let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
        for key in keys {
            let field = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
            match (a.get(key), b.get(key)) {
                (Some(a), Some(b)) => fields(out, collection, id, &field, a, b),
                (a, b) => entry(out, collection, id, &field, DifferenceKind::Changed, a, b),
            }
        }
    } else {
        entry(
            out,
            collection,
            id,
            path,
            DifferenceKind::Changed,
            Some(before),
            Some(after),
        );
    }
}
fn entities(out: &mut Vec<ProjectDifference>, name: &str, before: &Value, after: &Value) {
    let index = |v: &Value| -> BTreeMap<String, Value> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|v| (v["id"].as_str().unwrap().to_owned(), v.clone()))
            .collect()
    };
    let a = index(before);
    let b = index(after);
    for id in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
        match (a.get(id), b.get(id)) {
            (Some(a), Some(b)) => fields(out, name, Some(id), "", a, b),
            (a, b) => entry(
                out,
                name,
                Some(id),
                "",
                if a.is_none() {
                    DifferenceKind::Added
                } else {
                    DifferenceKind::Removed
                },
                a,
                b,
            ),
        }
    }
    // Array order is rendering order for tracks/clips. Insertions/removals alone are not reorders.
    if matches!(name, "tracks" | "clips") {
        let common_order = |v: &Value| -> Vec<String> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|v| v["id"].as_str().unwrap())
                .filter(|id| a.contains_key(*id) && b.contains_key(*id))
                .map(str::to_owned)
                .collect()
        };
        let first = common_order(before);
        let second = common_order(after);
        if first != second {
            entry(
                out,
                name,
                None,
                "/order",
                DifferenceKind::Reordered,
                Some(&serde_json::json!(first)),
                Some(&serde_json::json!(second)),
            );
        }
    }
}
pub fn compare_projects(
    before: &Project,
    after: &Project,
) -> Result<Vec<ProjectDifference>, ValidationError> {
    sungeki_project::validate(before)?;
    sungeki_project::validate(after)?;
    if before.id != after.id {
        return Err(ValidationError::new("別作品は比較できません".into()));
    }
    let a = serde_json::to_value(before).map_err(|e| ValidationError::new(e.to_string()))?;
    let b = serde_json::to_value(after).map_err(|e| ValidationError::new(e.to_string()))?;
    let mut output = Vec::new();
    for (name, value) in a.as_object().unwrap() {
        if matches!(
            name.as_str(),
            "tracks" | "scenes" | "groups" | "clips" | "assets" | "characters"
        ) {
            entities(&mut output, name, value, &b[name]);
        } else {
            fields(
                &mut output,
                "project",
                None,
                &format!("/{name}"),
                value,
                &b[name],
            );
        }
    }
    Ok(output)
}
