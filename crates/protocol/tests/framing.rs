use sungeki_protocol::{
    framing::{MAX_FRAME_BYTES, read_frame, write_frame},
    *,
};
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn fragmented_frame_round_trips_without_line_delimiters() {
    let request = Request {
        request_id: 42,
        body: RequestBody::Hello {
            hello: ClientHello::new("日本語\nGUI"),
        },
    };
    let mut bytes = Vec::new();
    write_frame(&mut bytes, &request).await.unwrap();
    let (mut writer, mut reader) = tokio::io::duplex(2);
    let send = tokio::spawn(async move {
        for byte in bytes {
            writer.write_all(&[byte]).await.unwrap();
        }
    });
    let result: Request = read_frame(&mut reader).await.unwrap();
    assert_eq!(result.request_id, 42);
    let RequestBody::Hello { hello } = result.body else {
        panic!("wrong response")
    };
    assert_eq!(hello.client_name, "日本語\nGUI");
    send.await.unwrap();
}

#[tokio::test]
async fn rejects_oversized_length_before_allocating_payload() {
    let bytes = ((MAX_FRAME_BYTES + 1) as u32).to_be_bytes();
    let error = read_frame::<Request>(&mut bytes.as_slice())
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ValidationFailed);
}

#[tokio::test]
async fn partial_payload_is_not_a_valid_request() {
    let bytes = [0, 0, 0, 20, b'{'];
    assert!(read_frame::<Request>(&mut bytes.as_slice()).await.is_err());
}

#[tokio::test]
async fn document_frame_boundary_and_handshake_limit_are_independent() {
    use sungeki_protocol::framing::{MAX_HANDSHAKE_BYTES, read_frame_limited};
    // JSON string quotes account for two bytes at the exact frame boundary.
    let value = "x".repeat(MAX_FRAME_BYTES - 2);
    let mut bytes = Vec::new();
    write_frame(&mut bytes, &value).await.unwrap();
    assert_eq!(bytes.len(), MAX_FRAME_BYTES + 4);
    let decoded: String = read_frame(&mut bytes.as_slice()).await.unwrap();
    assert_eq!(decoded, value);
    assert!(
        read_frame_limited::<String>(&mut bytes.as_slice(), MAX_HANDSHAKE_BYTES)
            .await
            .is_err()
    );
    let mut rejected = Vec::new();
    assert!(write_frame(&mut rejected, &(value + "x")).await.is_err());
    assert!(rejected.is_empty());
}

#[test]
fn unknown_fields_and_unknown_requests_are_rejected() {
    let unknown = r#"{"request_id":1,"body":{"type":"get_capabilities","hidden":true}}"#;
    assert!(serde_json::from_str::<Request>(unknown).is_err());
    let unknown = r#"{"request_id":1,"body":{"type":"edit_project"}}"#;
    assert!(serde_json::from_str::<Request>(unknown).is_err());
}
