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
fn peer_snapshots_and_expiration_have_deterministic_order() {
    let mut table = PeerTable::new();
    let now = Instant::now();
    let addr = "127.0.0.1:9001".parse().unwrap();
    for id in ["carol", "bob"] {
        table.register_or_update(id.into(), id.into(), addr, now);
    }
    assert_eq!(table.get_all().iter().map(|peer| peer.id.as_str()).collect::<Vec<_>>(), vec!["bob", "carol"]);
    assert_eq!(
        table.cleanup_expired(now + Duration::from_secs(7), Duration::from_secs(6)),
        vec![
            PeerEvent::Expired {
                id: "bob".into(),
                nickname: "bob".into()
            },
            PeerEvent::Expired {
                id: "carol".into(),
                nickname: "carol".into()
            },
        ]
    );
}
