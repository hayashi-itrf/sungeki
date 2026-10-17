use crate::{PsdPose, PsdProbe, PsdSelection, ValidationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PsdSpeechBinding {
    pub character_id: String,
    pub group: String,
    pub closed_option: String,
    pub open_option: String,
}
impl PsdSpeechBinding {
    pub fn pose(&self, base: &PsdPose, open: bool) -> PsdPose {
        let mut pose = base.clone();
        pose.selections.retain(|s| s.group != self.group);
        pose.selections.push(PsdSelection {
            group: self.group.clone(),
            option: if open {
                self.open_option.clone()
            } else {
                self.closed_option.clone()
            },
        });
        pose
    }
    pub fn validate(&self, base: &PsdPose, probe: &PsdProbe) -> Result<(), ValidationError> {
        if self.closed_option == self.open_option {
            return Err(ValidationError::new(
                "口の開閉には異なる選択肢が必要です".into(),
            ));
        }
        self.pose(base, false).validate(probe)?;
        self.pose(base, true).validate(probe)
    }
}
