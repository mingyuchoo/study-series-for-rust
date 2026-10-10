//! 같은 PC에서 두 노드가 서로를 발견하고, 채팅을 주고받고, GOODBYE로 퇴장하는지
//! 확인한다. 멀티캐스트 루프백이 필요하므로 네트워크 환경에 따라 실패할 수
//! 있다.

use async_channel::Receiver;
use p2p_app::{LeaveReason,
              NodeCommand,
              NodeConfig,
              NodeEvent,
              start};
use std::{thread,
          time::{Duration,
                 Instant}};

const TIMEOUT: Duration = Duration::from_secs(15);

/// 조건에 맞는 이벤트가 올 때까지 기다린다. 제한 시간을 넘기면 테스트를
/// 실패시킨다.
fn wait_for(events: &Receiver<NodeEvent>, mut accept: impl FnMut(&NodeEvent) -> bool) -> NodeEvent {
    let deadline = Instant::now() + TIMEOUT;
    loop {
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
    let alice = start(NodeConfig {
        nickname: "Alice".into(),
        listen_port: 19101,
    })
    .expect("Alice 노드 시작 실패");
    let bob = start(NodeConfig {
        nickname: "Bob".into(),
        listen_port: 19102,
    })
    .expect("Bob 노드 시작 실패");

    // 1. 서로 발견한다 (HELLO 멀티캐스트)
    wait_for(&bob.events, |e| matches!(e, NodeEvent::PeerJoined(p) if p.nickname == "Alice"));
    wait_for(&alice.events, |e| matches!(e, NodeEvent::PeerJoined(p) if p.nickname == "Bob"));

    // 2. 채팅을 보낸다 (CHAT 유니캐스트)
    alice.commands.try_send(NodeCommand::SendChat("안녕, Bob!".into())).expect("명령 전송 실패");
    match wait_for(&bob.events, |e| matches!(e, NodeEvent::ChatReceived { .. })) {
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

    // 3. 정상 종료하면 GOODBYE로 상대 화면에서 퇴장 처리된다
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
    bob.shutdown();
}
