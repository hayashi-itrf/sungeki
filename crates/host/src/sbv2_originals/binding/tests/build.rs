use super::startup::request;
use serde_json::json;

#[tokio::test]
async fn managed_build_sbv2_applies_and_cancels_through_parent() {
    for cancel in [false, true] {
        let (store, request, pid) = request(if cancel { "speech_hang" } else { "speech" });
        let profile = json!({"format_version":1,"profiles":[{
            "id":"build-sbv2","name":"Build SBV2","binding":request.binding,
            "cpu_only":true,"catalog":request.catalog.models()
        }]});
        let marker = store.0.join("voice.request");
        crate::build_record::managed_integration_tests::verify(
            store.open(),
            &store.0,
            sungeki_project::Voice::Sbv2 {
                settings: request.settings,
                identity: request.identity,
            },
            crate::build_record::profiles::Kind::Sbv2,
            profile,
            &pid,
            cancel.then_some(marker.as_path()),
        )
        .await;
    }
}
