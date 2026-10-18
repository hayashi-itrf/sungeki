use crate::{RenderError, plan::error};
use serde_json::Value;

pub(super) fn validate(
    report: &Value,
    font: &str,
    frame: u64,
    time: &Value,
    index: usize,
) -> Result<(), RenderError> {
    match report["layout_version"].as_u64() {
        Some(actual) if actual == u64::from(crate::CURRENT_LAYOUT_VERSION) => {}
        Some(actual) => {
            return Err(error(format!(
                "アプリと画像描画処理の版が一致しません（必要な版: {expected}、応答した版: {actual}）。処理を終了してアプリを再起動してください。解消しない場合はアプリを更新してください。",
                expected = crate::CURRENT_LAYOUT_VERSION
            )));
        }
        None => {
            return Err(error(
                "画像描画処理から正しい版情報が返されませんでした。アプリを再起動してください。",
            ));
        }
    }
    if report["font_sha256"] != font || report["frame_index"] != frame || report["time_us"] != *time
    {
        return Err(error(format!(
            "描画結果の識別情報が一致しません: 区間{index}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn incompatible_worker_reports_expected_and_observed_versions_with_recovery_steps() {
        let report = json!({"layout_version":7});
        let message = validate(&report, "font", 0, &json!(0), 0)
            .unwrap_err()
            .to_string();
        for text in ["必要な版: 10", "応答した版: 7", "再起動", "更新"] {
            assert!(message.contains(text), "{message}");
        }
    }

    #[test]
    fn malformed_version_is_distinct_from_mismatched_frame_identity() {
        for version in [Value::Null, json!("8"), json!(-1), json!(8.5)] {
            let message = validate(&json!({"layout_version":version}), "font", 0, &json!(0), 0)
                .unwrap_err()
                .to_string();
            assert!(message.contains("正しい版情報"));
        }
        let report = json!({"layout_version":crate::CURRENT_LAYOUT_VERSION,"font_sha256":"font","frame_index":0,"time_us":0});
        validate(&report, "font", 0, &json!(0), 0).unwrap();
        for (field, value) in [
            ("font_sha256", json!("other")),
            ("frame_index", json!(1)),
            ("time_us", json!(1)),
        ] {
            let mut wrong = report.clone();
            wrong[field] = value;
            let message = validate(&wrong, "font", 0, &json!(0), 3)
                .unwrap_err()
                .to_string();
            assert!(message.contains("識別情報"));
            assert!(!message.contains("起動し直し"));
        }
    }
}
