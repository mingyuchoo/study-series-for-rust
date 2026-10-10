use super::*;

#[test]
fn test_message_serialization() {
    let msg = Message::hello("id-123", "Charlie", 9003);
    let serialized = serde_json::to_string(&msg).unwrap();
    assert!(serialized.contains("\"type\":\"HELLO\""));
    assert!(serialized.contains("\"nickname\":\"Charlie\""));

    let deserialized: Message = serde_json::from_str(&serialized).unwrap();
    assert_eq!(msg, deserialized);
}

#[test]
fn test_encode_decode_roundtrip_with_korean_text() {
    let msg = Message::chat("id-456", "민규", "안녕하세요, 피어 채팅입니다.");
    let bytes = msg.encode().unwrap();
    assert_eq!(Message::decode(&bytes).unwrap(), msg);
}

#[test]
fn test_encode_rejects_oversized_message() {
    let msg = Message::chat("id-789", "Dave", "가".repeat(MAX_DATAGRAM_BYTES));
    assert!(matches!(msg.encode(), Err(MessageError::MessageTooLarge { .. })));
}

#[test]
fn test_decode_rejects_garbage() {
    assert!(Message::decode(b"not json").is_err());
}
