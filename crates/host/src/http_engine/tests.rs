use super::*;
use std::net::TcpListener;
#[test]
fn occupied_port_is_never_adopted_or_spawned() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!(
        "http://127.0.0.1:{}/",
        listener.local_addr().unwrap().port()
    );
    let result = launch::command(
        Engine::Voicevox,
        &endpoint,
        std::path::Path::new("/missing/engine"),
        &Default::default(),
    )
    .unwrap();
    assert!(result.is_none());
    assert!(listener.local_addr().is_ok());
}
#[test]
fn launch_rejects_remote_or_invalid_coeiroink_endpoint() {
    for (engine, url) in [
        (Engine::Voicevox, "http://192.0.2.1:50021/"),
        (Engine::Coeiroink, "http://127.0.0.1:50033/"),
        (Engine::Aivis, "http://127.0.0.1:80/"),
    ] {
        assert!(
            launch::command(
                engine,
                url,
                std::path::Path::new("/missing/engine"),
                &Default::default()
            )
            .is_err()
        );
    }
}
#[test]
fn optional_paths_preserve_default_manual_start() {
    let value = sungeki_protocol::RuntimeSettings::default();
    assert!(
        value.voicevox_engine.is_none()
            && value.aivis_engine.is_none()
            && value.coeiroink_engine.is_none()
    );
}
