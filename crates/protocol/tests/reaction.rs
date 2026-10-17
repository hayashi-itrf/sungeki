use sungeki_protocol::SessionRequest;
#[test]
fn reaction_command_requires_minor_98_and_serializes_explicit_geometry() {
    let request: SessionRequest = serde_json::from_value(serde_json::json!({
        "type":"preview_edit", "session_id":"s", "expected_revision":0,"offset":0,"limit":10,
        "commands":[{"type":"set_reaction_motion","clip_id":"c","center_x":320,"center_y":190,
            "width":120,"height":120,"start_scale":0.6,"fade":{"in_us":80000,"out_us":150000}}]
    }))
    .unwrap();
    assert_eq!(request.minimum_protocol_minor(), 98);
    let encoded = serde_json::to_value(&request).unwrap();
    assert_eq!(encoded["commands"][0]["center_x"], 320.);
    assert_eq!(
        serde_json::from_value::<SessionRequest>(encoded)
            .unwrap()
            .minimum_protocol_minor(),
        98
    );
}
