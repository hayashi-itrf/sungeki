use crate::FramePreviewOutput;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::{Check, TimeUs};
use ts_rs::TS;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewSampleReason {
    Start,
    End,
    SceneMiddle { scene_id: String },
    BeforeSceneStart { scene_id: String },
    SceneStart { scene_id: String },
    BeforeSceneEnd { scene_id: String },
    SceneEnd { scene_id: String },
    Requested { time_us: TimeUs },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewFrameSample {
    #[ts(type = "number")]
    pub frame_index: u64,
    pub time_us: TimeUs,
    pub reasons: Vec<ReviewSampleReason>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewUnsampledSpan {
    /// Half-open frame range, exact even at fractional FPS.
    #[ts(type = "number")]
    pub first: u64,
    #[ts(type = "number")]
    pub end: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewFrameSampling {
    #[ts(type = "number")]
    pub frame_count: u64,
    pub selected: Vec<ReviewFrameSample>,
    /// Candidate frames omitted by the limit, including their reasons.
    pub omitted: Vec<ReviewFrameSample>,
    /// All frames outside `selected`; selected frames are also uninspected until rendered/reviewed.
    pub unsampled: Vec<ReviewUnsampledSpan>,
    pub scenes_without_visible_frame: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewFrame {
    pub sample: ReviewFrameSample,
    pub output: Option<FramePreviewOutput>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewContactSheet {
    pub page: u32,
    #[ts(type = "number[]")]
    pub frame_indices: Vec<u64>,
    pub output: Option<FramePreviewOutput>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewFrames {
    pub sampling: ReviewFrameSampling,
    pub frames: Vec<ReviewFrame>,
    pub contact_sheets: Vec<ReviewContactSheet>,
    pub check: Check,
    pub visual_review: Check,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewImageTarget {
    Frame {
        #[ts(type = "number")]
        frame_index: u64,
    },
    ContactSheet {
        page: u32,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReadReviewImageArgs {
    pub job_id: String,
    pub target: ReviewImageTarget,
    #[ts(type = "number")]
    pub offset: u64,
    pub length: u32,
}

#[cfg(test)]
mod access_tests {
    use super::*;
    #[test]
    fn review_image_access_requires_minor74_and_rejects_paths_and_ambiguous_selectors() {
        let value = serde_json::json!({"job_id":"j", "target":{"type":"contact_sheet","page":1}, "offset":0, "length":1});
        let args: ReadReviewImageArgs = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            crate::SessionRequest::ReadReviewImage { args }.minimum_protocol_minor(),
            74
        );
        for altered in [
            serde_json::json!({"job_id":"j", "target":{"type":"frame","frame_index":0,"page":1}, "offset":0, "length":1}),
            serde_json::json!({"job_id":"j", "target":{"type":"frame","frame_index":-1}, "offset":0, "length":1}),
            serde_json::json!({"job_id":"j", "target":{"type":"contact_sheet","page":1}, "offset":0, "length":1,"path":"/arbitrary/file"}),
        ] {
            assert!(serde_json::from_value::<ReadReviewImageArgs>(altered).is_err());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewFrameOptions {
    pub requested_times_us: Vec<TimeUs>,
    pub max_frames: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text_safe_margin_percent: Option<u8>,
}
impl Default for ReviewFrameOptions {
    fn default() -> Self {
        Self {
            requested_times_us: vec![],
            max_frames: 12,
            text_safe_margin_percent: None,
        }
    }
}

#[cfg(test)]
mod margin_tests {
    use super::*;
    #[test]
    fn margin_option_is_explicit_and_requires_protocol_76() {
        let mut options = ReviewFrameOptions::default();
        assert!(
            serde_json::to_value(&options)
                .unwrap()
                .get("text_safe_margin_percent")
                .is_none()
        );
        let request = |options| crate::SessionRequest::StartReview {
            session_id: "s".into(),
            job_id: "j".into(),
            expected_revision: 0,
            wait_ms: 0,
            sampling: Some(options),
        };
        assert_eq!(request(options.clone()).minimum_protocol_minor(), 75);
        for margin in [0, 5, 25] {
            options.text_safe_margin_percent = Some(margin);
            assert_eq!(request(options.clone()).minimum_protocol_minor(), 76);
            assert_eq!(
                serde_json::from_value::<ReviewFrameOptions>(
                    serde_json::to_value(&options).unwrap()
                )
                .unwrap(),
                options
            );
        }
        for value in [
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!(256),
            serde_json::json!("5"),
        ] {
            let mut json = serde_json::to_value(&options).unwrap();
            json["text_safe_margin_percent"] = value;
            assert!(serde_json::from_value::<ReviewFrameOptions>(json).is_err());
        }
    }
}
