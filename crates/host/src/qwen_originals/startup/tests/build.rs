use super::request;
use crate::speech_journal_tests::Fixture;
use serde_json::json;

#[tokio::test]
async fn managed_build_qwen_applies_and_cancels_through_parent() {
    for cancel in [false, true] {
        let (engine, request, pid) = request(if cancel { "speech_hang" } else { "speech" });
        let store = Fixture::new();
        let profile = json!({"format_version":1,"profiles":[{
            "id":"build-qwen","name":"Build Qwen","binding":request.binding,
            "port":request.port,"identity":request.identity,"default_settings":request.settings,
            "catalog":[
                {"status":"healthy","backend":{"name":"official","model_id":request.identity.model_id,"ready":true},"version":"test"},
                {"data":[{"id":"qwen3-tts"}]},
                {"voices":[{"id":"Vivian","name":"Vivian"}],"languages":["Japanese"]}
            ]
        }]});
        let marker = engine.0.join("speech.started");
        crate::build_record::managed_integration_tests::verify(
            store.open(),
            &store.0,
            sungeki_project::Voice::Qwen {
                settings: request.settings,
                identity: request.identity,
            },
            crate::build_record::profiles::Kind::Qwen,
            profile,
            &pid,
            cancel.then_some(marker.as_path()),
        )
        .await;
    }
}
