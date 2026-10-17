//! Immutable history state and its canonical JSON travel together across candidate clones.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;
use std::sync::Arc;
use sungeki_project::{Project, ValidationError};

struct Inner {
    project: Project,
    json: Box<RawValue>,
}
#[derive(Clone)]
pub(super) struct CachedProject(Arc<Inner>);
impl CachedProject {
    pub(super) fn new(project: Project) -> Result<Self, ValidationError> {
        let json = serde_json::value::to_raw_value(&project)
            .map_err(|e| ValidationError::new(e.to_string()))?;
        Ok(Self(Arc::new(Inner { project, json })))
    }
    pub(super) fn project(&self) -> &Project {
        &self.0.project
    }
    pub(super) fn bytes(&self) -> usize {
        self.0.json.get().len()
    }
    pub(super) fn into_project(self) -> Project {
        match Arc::try_unwrap(self.0) {
            Ok(inner) => inner.project,
            Err(shared) => shared.project.clone(),
        }
    }
}
impl Serialize for CachedProject {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.json.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for CachedProject {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let project = Project::deserialize(deserializer)?;
        Self::new(project).map_err(|e| serde::de::Error::custom(e.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloned_history_shares_immutable_data_and_serializes_identically() {
        let project = Project::empty(uuid::Uuid::new_v4().to_string(), "日本語 \"履歴\"\n".into());
        let expected = serde_json::to_vec(&project).unwrap();
        let cached = CachedProject::new(project).unwrap();
        let other = cached.clone();
        assert!(Arc::ptr_eq(&cached.0, &other.0));
        assert_eq!(serde_json::to_vec(&cached).unwrap(), expected);
        let mut candidate = other.into_project();
        candidate.title = "changed".into();
        assert_eq!(serde_json::to_vec(&cached).unwrap(), expected);
        let restored: CachedProject = serde_json::from_slice(&expected).unwrap();
        assert_eq!(serde_json::to_vec(&restored).unwrap(), expected);
        assert_eq!(restored.bytes(), expected.len());
    }
}
