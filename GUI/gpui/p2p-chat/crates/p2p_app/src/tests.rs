use super::*;
use p2p_core::{MAX_DATAGRAM_BYTES,
               Message};
use std::{net::SocketAddr,
          time::{Duration,
                 Instant,
                 SystemTime}};

fn node() -> Node { Node::new(NodeConfig::new("Alice", 9001).unwrap(), "alice".into()).unwrap() }

fn inbound(message: Message) -> Inbound {
    Inbound {
        from: "127.0.0.1:50000".parse().unwrap(),
        message,
    }
}

fn discover(node: &mut Node, now: Instant) { node.receive(inbound(Message::hello("bob", "Bob", 9002)), now, SystemTime::UNIX_EPOCH); }

#[test]
fn configuration_is_validated_for_every_caller() {
    assert_eq!(NodeConfig::new("   ", 9001), Err(ConfigError::EmptyNickname));
    assert_eq!(NodeConfig::new("가".repeat(21), 9001), Err(ConfigError::NicknameTooLong));
    assert_eq!(NodeConfig::new("Alice", 0), Err(ConfigError::InvalidPort));
    assert_eq!(NodeConfig::new(" 가 ", 65535).unwrap().nickname, "가");
    assert!(NodeConfig::new("가".repeat(20), 1).is_ok());
    assert!(matches!(
        Node::new(
            NodeConfig {
                nickname: " ".into(),
                listen_port: 9001
            },
            "alice".into()
        ),
        Err(ConfigError::EmptyNickname)
    ));
}

#[test]
fn discovery_updates_advertised_endpoint_and_goodbye_removes_peer() {
    let mut node = node();
    let now = Instant::now();
    assert_eq!(node.hello(), Message::hello("alice", "Alice", 9001));
    assert_eq!(node.goodbye(), Message::goodbye("alice", "Alice"));
    assert_eq!(
        node.receive(inbound(Message::hello("bob", "Bob", 9002)), now, SystemTime::UNIX_EPOCH),
        Some(NodeEvent::PeerJoined(PeerInfo {
            id: "bob".into(),
            nickname: "Bob".into(),
            endpoint: "127.0.0.1:9002".parse().unwrap(),
        }))
    );
    assert!(
        matches!(node.receive(inbound(Message::hello("bob", "Robert", 9010)), now, SystemTime::UNIX_EPOCH), Some(NodeEvent::PeerUpdated(peer)) if peer.nickname == "Robert")
    );
    assert_eq!(node.prepare_chat(" hi ").unwrap().targets(), &["127.0.0.1:9010".parse::<SocketAddr>().unwrap()]);
    assert_eq!(
        node.receive(inbound(Message::goodbye("bob", "Robert")), now, SystemTime::UNIX_EPOCH),
        Some(NodeEvent::PeerLeft {
            id: "bob".into(),
            nickname: "Robert".into(),
            reason: LeaveReason::Goodbye,
        })
    );
    assert!(node.prepare_chat("hi").is_err());
    assert_eq!(node.receive(inbound(Message::goodbye("bob", "Robert")), now, SystemTime::UNIX_EPOCH), None);
}

#[test]
fn own_messages_are_ignored_and_chat_uses_supplied_time() {
    let mut node = node();
    let now = Instant::now();
    let at = SystemTime::UNIX_EPOCH + Duration::from_secs(42);
    assert_eq!(node.receive(inbound(Message::hello("alice", "Alice", 9001)), now, at), None);
    assert_eq!(node.receive(inbound(Message::chat("alice", "Alice", "echo")), now, at), None);
    assert_eq!(
        node.receive(inbound(Message::chat("bob", "Bob", "안녕")), now, at),
        Some(NodeEvent::ChatReceived {
            id: "bob".into(),
            nickname: "Bob".into(),
            content: "안녕".into(),
            at,
        })
    );
}

#[test]
fn expiration_uses_injected_time_and_hello_refreshes_deadline() {
    let mut node = node();
    let now = Instant::now();
    discover(&mut node, now);
    assert!(node.expire(now + PEER_TIMEOUT).is_empty());
    discover(&mut node, now + Duration::from_secs(5));
    assert!(node.expire(now + Duration::from_secs(7)).is_empty());
    assert_eq!(
        node.expire(now + Duration::from_secs(12)),
        vec![NodeEvent::PeerLeft {
            id: "bob".into(),
            nickname: "Bob".into(),
            reason: LeaveReason::Timeout,
        }]
    );
    assert!(node.expire(now + Duration::from_secs(13)).is_empty());
    assert!(node.prepare_chat("hi").is_err());
}

#[test]
fn chat_validation_preserves_errors_and_prepares_all_targets() {
    let mut node = node();
    assert_eq!(node.prepare_chat(" \n ").unwrap_err(), "빈 메시지는 보낼 수 없습니다.");
    assert_eq!(node.prepare_chat("hi").unwrap_err(), "연결된 피어가 없습니다.");
    assert!(node.prepare_chat(&"가".repeat(MAX_DATAGRAM_BYTES)).unwrap_err().contains("Message too large"));
    let now = Instant::now();
    discover(&mut node, now);
    node.receive(inbound(Message::hello("carol", "Carol", 9003)), now, SystemTime::UNIX_EPOCH);
    let dispatch = node.prepare_chat(" 안녕 ").unwrap();
    assert_eq!(dispatch.message(), &Message::chat("alice", "Alice", "안녕"));
    let mut targets = dispatch.targets().to_vec();
    targets.sort();
    assert_eq!(
        targets,
        vec!["127.0.0.1:9002".parse::<SocketAddr>().unwrap(), "127.0.0.1:9003".parse().unwrap()]
    );
    assert_eq!(
        dispatch.finish(1, "one peer failed".into(), SystemTime::UNIX_EPOCH),
        NodeEvent::ChatSent {
            content: "안녕".into(),
            sent_to: 1,
            at: SystemTime::UNIX_EPOCH,
        }
    );
    assert_eq!(
        node.prepare_chat("hi").unwrap().finish(0, "offline".into(), SystemTime::UNIX_EPOCH),
        NodeEvent::SendFailed("offline".into())
    );
}
