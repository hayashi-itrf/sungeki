use sungeki_protocol::SessionRequest;
#[test]
fn audio_probe_requires_minor_41() {
    assert_eq!(
        SessionRequest::ProbeAudio {
            path: "/audio.wav".into()
        }
        .minimum_protocol_minor(),
        41
    );
}
