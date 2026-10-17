use crate::{AssetProbe, Project, TimeUs, ValidationError, VrmKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// One looping VRMA; source time is measured from this clip's original start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VrmStanding {
    pub model_asset_id: String,
    pub motion_asset_id: String,
    pub source_start_us: TimeUs,
    pub expressions: BTreeMap<String, f64>,
    pub character_id: Option<String>,
    /// Absolute normalized humanoid rotations, XYZ Euler degrees. Unspecified joints follow motion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pose: Option<BTreeMap<String, [f64; 3]>>,
}
impl VrmStanding {
    pub fn validate(&self, project: &Project, duration: TimeUs) -> Result<(), ValidationError> {
        if let Some(pose) = &self.pose {
            validate_pose(pose)?;
        }
        let fail = || ValidationError::new("VRM立ち絵の素材・表情・話者・時刻が不正です".into());
        let find = |id: &str, kind| {
            project.assets.iter().find(|a| a.id == id).and_then(|a| {
                if let AssetProbe::Vrm(p) = &a.probe {
                    (p.inspection.kind == kind).then_some(p)
                } else {
                    None
                }
            })
        };
        let model = find(&self.model_asset_id, VrmKind::Model).ok_or_else(fail)?;
        let motion = find(&self.motion_asset_id, VrmKind::Motion).ok_or_else(fail)?;
        model.validate()?;
        motion.validate()?;
        if self
            .source_start_us
            .0
            .checked_add(duration.0)
            .is_none_or(|t| t > crate::MAX_TIME_US)
            || self.expressions.len() > 256
            || self.expressions.iter().any(|(name, value)| {
                !value.is_finite()
                    || !(0.0..=1.0).contains(value)
                    || !model.inspection.expressions.contains(name)
            })
            || self
                .character_id
                .as_ref()
                .is_some_and(|id| !project.characters.iter().any(|c| &c.id == id))
        {
            return Err(fail());
        }
        Ok(())
    }
}

/// Same twenty major humanoid joints exposed in the pose editor.
pub fn validate_pose(pose: &BTreeMap<String, [f64; 3]>) -> Result<(), ValidationError> {
    const BONES: &[&str] = &[
        "hips",
        "spine",
        "chest",
        "upperChest",
        "neck",
        "head",
        "leftShoulder",
        "leftUpperArm",
        "leftLowerArm",
        "leftHand",
        "rightShoulder",
        "rightUpperArm",
        "rightLowerArm",
        "rightHand",
        "leftUpperLeg",
        "leftLowerLeg",
        "leftFoot",
        "rightUpperLeg",
        "rightLowerLeg",
        "rightFoot",
    ];
    if pose.iter().any(|(name, angles)| {
        !BONES.contains(&name.as_str())
            || angles
                .iter()
                .any(|v| !v.is_finite() || !(-180.0..=180.0).contains(v))
    }) {
        return Err(ValidationError::new(
            "VRMの関節名または角度（−180〜180度）が不正です".into(),
        ));
    }
    Ok(())
}
