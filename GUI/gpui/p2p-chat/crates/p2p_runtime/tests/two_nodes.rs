//! 같은 PC에서 두 노드가 서로를 발견하고, 채팅을 주고받고, GOODBYE로 퇴장하는지
//! 확인한다. 멀티캐스트 루프백이 필요하므로 네트워크 환경에 따라 실패할 수
//! 있다.

use async_channel::Receiver;
use p2p_app::{LeaveReason,
              NodeCommand,
              NodeConfig,
              NodeEvent};
use p2p_runtime::start_with_state_dir;
use std::{thread,
          time::{Duration,
                 Instant}};

const TIMEOUT: Duration = Duration::from_secs(15);

/// 조건에 맞는 이벤트가 올 때까지 기다린다. 제한 시간을 넘기면 테스트를
/// 실패시킨다.
fn wait_for(events: &Receiver<NodeEvent>, mut accept: impl FnMut(&NodeEvent) -> bool) -> NodeEvent {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        assert!(Instant::now() < deadline, "제한 시간 안에 기대한 이벤트가 오지 않았습니다");
        match events.try_recv() {
            | Ok(event) if accept(&event) => return event,
            | Ok(_) => {},
            | Err(_) => {
                assert!(Instant::now() < deadline, "제한 시간 안에 기대한 이벤트가 오지 않았습니다");
                thread::sleep(Duration::from_millis(20));
            },
        }
    }
}

#[test]
fn two_nodes_discover_chat_and_say_goodbye() {
    let state = tempfile::tempdir().unwrap();
    let alice = start_with_state_dir(
        NodeConfig {
            nickname: "Alice".into(),
            listen_port: 19101,
        },
        state.path(),
    )
    .expect("Alice 노드 시작 실패");
    let bob = start_with_state_dir(
        NodeConfig {
            nickname: "Bob".into(),
            listen_port: 19102,
        },
        state.path(),
    )
    .expect("Bob 노드 시작 실패");

    // 1. 서로 발견한다 (HELLO 멀티캐스트)
    wait_for(&bob.events, |e| matches!(e, NodeEvent::PeerJoined(p) if p.id == alice.node_id));
    wait_for(&alice.events, |e| matches!(e, NodeEvent::PeerJoined(p) if p.id == bob.node_id));

    // 2. 인증된 QUIC 스트림으로 채팅을 주고받는다.
    alice.commands.try_send(NodeCommand::SendChat("안녕, Bob!".into())).expect("명령 전송 실패");
    match wait_for(&bob.events, |e| matches!(e, NodeEvent::ChatReceived { id, .. } if id == &alice.node_id)) {
        | NodeEvent::ChatReceived {
            nickname,
            content,
            ..
        } => {
            assert_eq!(nickname, "Alice");
            assert_eq!(content, "안녕, Bob!");
        },
        | other => panic!("예상하지 못한 이벤트: {other:?}"),
    }
    wait_for(&alice.events, |e| matches!(e, NodeEvent::ChatSent { .. }));

    for i in 0 .. 5 {
        bob.commands.try_send(NodeCommand::SendChat(format!("답장 {i}"))).unwrap();
    }
    for i in 0 .. 5 {
        assert!(
            matches!(wait_for(&alice.events, |e| matches!(e, NodeEvent::ChatReceived { id, .. } if id == &bob.node_id)),
            NodeEvent::ChatReceived { content, .. } if content == format!("답장 {i}"))
        );
    }

    // 3. 정상 종료하면 GOODBYE로 상대 화면에서 퇴장 처리된다
    let alice_id = alice.node_id.clone();
    alice.shutdown();
    wait_for(&bob.events, |e| {
        matches!(
            e,
            NodeEvent::PeerLeft {
                reason: LeaveReason::Goodbye,
                ..
            }
        )
    });
    let alice = start_with_state_dir(NodeConfig::new("Alice", 19101).unwrap(), state.path()).unwrap();
    assert_eq!(alice.node_id, alice_id, "재시작해도 저장된 인증서와 노드 ID를 유지한다");
    wait_for(&bob.events, |e| matches!(e, NodeEvent::PeerJoined(p) if p.id == alice_id));
    wait_for(&alice.events, |e| matches!(e, NodeEvent::PeerJoined(p) if p.id == bob.node_id));
    bob.commands.try_send(NodeCommand::SendChat("재연결".into())).unwrap();
    assert!(matches!(
        wait_for(
            &alice.events,
            |e| matches!(e, NodeEvent::ChatReceived { id, content, .. } if id == &bob.node_id && content == "재연결")
        ),
        NodeEvent::ChatReceived { .. }
    ));
    alice.shutdown();
    bob.shutdown();
}

#[tokio::test]
async fn unreachable_quic_peer_does_not_block_a_real_nodes_inbound_chat() {
    use p2p_app::NetworkPort;
    use p2p_core::Message;
    use p2p_infra::QuicNetworkAdapter;
    use std::net::UdpSocket;

    let state = tempfile::tempdir().unwrap();
    let alice = start_with_state_dir(NodeConfig::new("Alice", 19111).unwrap(), state.path()).unwrap();
    let bob = start_with_state_dir(NodeConfig::new("Bob", 19112).unwrap(), state.path()).unwrap();
    wait_for(&alice.events, |e| matches!(e, NodeEvent::PeerJoined(p) if p.id == bob.node_id));
    wait_for(&bob.events, |e| matches!(e, NodeEvent::PeerJoined(p) if p.id == alice.node_id));

    let (slow, _inbound) = QuicNetworkAdapter::bind(0, state.path()).await.unwrap();
    let blackhole = UdpSocket::bind("0.0.0.0:0").unwrap();
    slow.broadcast_discovery(&Message::hello(slow.node_id(), "Slow", blackhole.local_addr().unwrap().port()))
        .await
        .unwrap();
    wait_for(&alice.events, |e| matches!(e, NodeEvent::PeerJoined(p) if p.id == slow.node_id()));

    let started = Instant::now();
    alice.commands.try_send(NodeCommand::SendChat("hello".into())).unwrap();
    wait_for(
        &bob.events,
        |e| matches!(e, NodeEvent::ChatReceived { id, content, .. } if id == &alice.node_id && content == "hello"),
    );
    bob.commands.try_send(NodeCommand::SendChat("reply".into())).unwrap();
    wait_for(
        &alice.events,
        |e| matches!(e, NodeEvent::ChatReceived { id, content, .. } if id == &bob.node_id && content == "reply"),
    );
    assert!(started.elapsed() < Duration::from_secs(1), "느린 peer의 3초 타임아웃 전에 수신을 처리해야 한다");
    alice.shutdown();
    bob.shutdown();
}
