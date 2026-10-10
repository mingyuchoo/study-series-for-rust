use super::*;

#[test]
fn test_peer_registration_and_update() {
    let mut table = PeerTable::new();
    let now = Instant::now();
    let addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();

    // 1. 신규 등록
    let event = table.register_or_update("node-1".into(), "Alice".into(), addr, now);
    match event {
        | PeerEvent::Discovered(peer) => {
            assert_eq!(peer.id, "node-1");
            assert_eq!(peer.nickname, "Alice");
            assert_eq!(peer.endpoint, addr);
        },
        | _ => panic!("Expected Discovered event"),
    }
    assert_eq!(table.count(), 1);

    // 2. 닉네임 갱신
    let event2 = table.register_or_update("node-1".into(), "Alice_Renamed".into(), addr, now);
    match event2 {
        | PeerEvent::Updated(peer) => {
            assert_eq!(peer.nickname, "Alice_Renamed");
        },
        | _ => panic!("Expected Updated event"),
    }
    assert_eq!(table.count(), 1);
}

#[test]
fn test_peer_expiration() {
    let mut table = PeerTable::new();
    let t0 = Instant::now();
    let addr: SocketAddr = "127.0.0.1:9002".parse().unwrap();

    table.register_or_update("node-2".into(), "Bob".into(), addr, t0);

    // 5초 경과 (타임아웃 6초 이전)
    let t1 = t0 + Duration::from_secs(5);
    let expired = table.cleanup_expired(t1, Duration::from_secs(6));
    assert!(expired.is_empty());
    assert_eq!(table.count(), 1);

    // 7초 경과 (타임아웃 6초 초과)
    let t2 = t0 + Duration::from_secs(7);
    let expired2 = table.cleanup_expired(t2, Duration::from_secs(6));
    assert_eq!(expired2.len(), 1);
    assert_eq!(table.count(), 0);
}

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
    assert!(matches!(msg.encode(), Err(P2PError::MessageTooLarge { .. })));
}

#[test]
fn test_decode_rejects_garbage() {
    assert!(Message::decode(b"not json").is_err());
}
