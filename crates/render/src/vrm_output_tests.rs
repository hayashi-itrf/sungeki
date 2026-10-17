use super::*;
use crate::vrm_request::{VrmFrameRequest, VrmSource};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
struct Fixture {
    root: PathBuf,
    output: PathBuf,
    request: VrmRenderRequest,
    report: Value,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("vrm-result-{}", uuid::Uuid::new_v4()));
        let output = root.join("render-fixture");
        fs::create_dir_all(&output).unwrap();
        let source = VrmSource {
            path: "/fixture/source".into(),
            sha256: "a".repeat(64),
            byte_length: 100,
        };
        let request = VrmRenderRequest {
            version: 1,
            model: source.clone(),
            motion: source.clone(),
            width: 1,
            height: 1,
            frames: vec![VrmFrameRequest {
                time_us: sungeki_project::TimeUs(123),
                pose: Default::default(),
                expressions: [("aa".into(), 1.0)].into(),
            }],
        };
        let png = include_bytes!("../../review/tests/data/transparent.png");
        fs::write(output.join("frame-000000.png"), png).unwrap();
        let input = json!({"path":source.path,"sha256":source.sha256,"bytes":100});
        let report = json!({"status":"passed","cleanupPassed":true,"remainingProcesses":[],"requestVersion":1,
            "width":1,"height":1,"inputs":{"model":input,"motion":input},
            "frames":[{"index":0,"time_us":123,"expressions":{"aa":1.0},"filename":"frame-000000.png","bytes":png.len(),"sha256":format!("{:x}", Sha256::digest(png))}]});
        Self {
            root,
            output,
            request,
            report,
        }
    }
    fn run(&self, report: &Value, n: usize) -> Result<Vec<VrmFrameOutput>, RenderError> {
        fs::write(
            self.output.join("report.json"),
            serde_json::to_vec(report).unwrap(),
        )
        .unwrap();
        let destination = self.root.join(format!("capture-{n}"));
        fs::create_dir(&destination).unwrap();
        capture(
            &self.request,
            &self.output,
            &self.root,
            &destination,
            &AtomicBool::new(false),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn receipt_identity_completion_and_png_bytes_are_required() {
    let f = Fixture::new();
    assert_eq!(f.run(&f.report, 0).unwrap().len(), 1);
    for (n, (pointer, value)) in [
        ("/status", json!("failed")),
        ("/cleanupPassed", json!(false)),
        ("/remainingProcesses", json!([123])),
        ("/requestVersion", json!(2)),
        ("/width", json!(2)),
        ("/inputs/model/sha256", json!("b".repeat(64))),
        ("/frames/0/time_us", json!(124)),
        ("/frames/0/index", json!(1)),
        ("/frames/0/expressions/aa", json!(0.0)),
        ("/frames/0/filename", json!("../other.png")),
        ("/frames/0/sha256", json!("b".repeat(64))),
        ("/frames", json!([])),
    ]
    .into_iter()
    .enumerate()
    {
        let mut report = f.report.clone();
        *report.pointer_mut(pointer).unwrap() = value;
        assert!(f.run(&report, n + 1).is_err(), "{pointer}");
    }
}
#[test]
fn pose_must_match_requested_joint_angles() {
    let mut f = Fixture::new();
    f.request.frames[0]
        .pose
        .insert("leftUpperArm".into(), [0.0, 0.0, 60.0]);
    assert!(f.run(&f.report, 0).is_err());
    f.report["frames"][0]["pose"] = json!({"leftUpperArm": [0.0, 0.0, 60.0]});
    assert_eq!(f.run(&f.report, 1).unwrap().len(), 1);
    f.report["frames"][0]["pose"]["leftUpperArm"] = json!([0.0, 0.0, 30.0]);
    assert!(f.run(&f.report, 2).is_err());
}
#[test]
fn matching_receipt_does_not_hide_wrong_dimensions_or_symlink_image() {
    let mut f = Fixture::new();
    f.request.width = 2;
    f.report["width"] = json!(2);
    assert!(f.run(&f.report, 0).is_err());
    f.request.width = 1;
    f.report["width"] = json!(1);
    fs::rename(
        f.output.join("frame-000000.png"),
        f.output.join("original.png"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        f.output.join("original.png"),
        f.output.join("frame-000000.png"),
    )
    .unwrap();
    assert!(f.run(&f.report, 1).is_err());
}
