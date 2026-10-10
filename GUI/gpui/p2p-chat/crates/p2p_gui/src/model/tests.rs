use super::*;
use std::time::Duration;

fn peer(nickname: &str) -> PeerInfo {
    PeerInfo {
        id: "bob".into(),
        nickname: nickname.into(),
        endpoint: "127.0.0.1:9002".parse().unwrap(),
    }
}

#[test]
fn peer_updates_do_not_append_history_and_notices_use_injected_time() {
    let mut model = Conversation::default();
    let at = SystemTime::UNIX_EPOCH + Duration::from_secs(42);
    assert!(model.apply(NodeEvent::PeerJoined(peer("Bob")), at));
    assert_eq!(
        model.entries,
        vec![ChatEntry::Notice {
            text: "'Bob' 입장".into(),
            at
        }]
    );
    assert!(!model.apply(NodeEvent::PeerUpdated(peer("Robert")), at));
    assert_eq!(model.peers, vec![peer("Robert")]);
    assert_eq!(model.entries.len(), 1);
    assert!(model.apply(
        NodeEvent::PeerLeft {
            id: "bob".into(),
            nickname: "Robert".into(),
            reason: LeaveReason::Timeout
        },
        at
    ));
    assert!(model.peers.is_empty());
    assert_eq!(
        model.entries.last(),
        Some(&ChatEntry::Notice {
            text: "'Robert' 응답 없음 (연결 끊김)".into(),
            at
        })
    );
    assert!(model.apply(
        NodeEvent::PeerLeft {
            id: "bob".into(),
            nickname: "Robert".into(),
            reason: LeaveReason::Goodbye
        },
        at
    ));
    assert_eq!(
        model.entries.last(),
        Some(&ChatEntry::Notice {
            text: "'Robert' 퇴장".into(),
            at
        })
    );
}

#[test]
fn chat_uses_event_time_while_errors_and_stop_do_not_append_history() {
    let mut model = Conversation::default();
    let at = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    assert!(!model.apply(NodeEvent::PeerUpdated(peer("Bob")), at));
    assert_eq!(model.peers.len(), 1);
    assert!(model.apply(
        NodeEvent::ChatReceived {
            id: "bob".into(),
            nickname: "Bob".into(),
            content: "hi".into(),
            at
        },
        SystemTime::UNIX_EPOCH
    ));
    assert_eq!(
        model.entries[0],
        ChatEntry::Message {
            speaker: Speaker::Peer {
                id: "bob".into(),
                nickname: "Bob".into()
            },
            content: "hi".into(),
            at
        }
    );
    assert!(model.apply(
        NodeEvent::ChatSent {
            content: "안녕".into(),
            sent_to: 1,
            at
        },
        SystemTime::UNIX_EPOCH
    ));
    assert_eq!(
        model.entries[1],
        ChatEntry::Message {
            speaker: Speaker::Me,
            content: "안녕".into(),
            at
        }
    );
    assert!(!model.apply(NodeEvent::SendFailed("offline".into()), at));
    assert!(!model.apply(NodeEvent::Stopped, at));
    assert_eq!(model.error.as_deref(), Some("offline"));
    assert!(model.stopped);
    assert_eq!(model.entries.len(), 2);
}
