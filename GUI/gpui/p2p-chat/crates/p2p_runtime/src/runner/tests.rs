use super::*;
use async_trait::async_trait;
use p2p_app::NodeConfig;
use p2p_core::Message;
use std::{io,
          net::SocketAddr,
          sync::{Arc,
                 Mutex}};

#[derive(Clone, Default)]
struct RecordingNetwork {
    broadcasts: Arc<Mutex<Vec<Message>>>,
    direct: Arc<Mutex<Vec<(SocketAddr, Message)>>>,
    fail_port: Option<u16>,
    stall_port: Option<u16>,
}

#[async_trait]
impl NetworkPort for RecordingNetwork {
    type Error = io::Error;

    async fn broadcast_discovery(&self, message: &Message) -> Result<(), Self::Error> {
        self.broadcasts.lock().unwrap().push(message.clone());
        Ok(())
    }

    async fn send_direct(&self, target: SocketAddr, message: &Message) -> Result<(), Self::Error> {
        self.direct.lock().unwrap().push((target, message.clone()));
        if self.stall_port == Some(target.port()) {
            std::future::pending::<()>().await;
        }
        if self.fail_port == Some(target.port()) {
            return Err(io::Error::other("offline"));
        }
        Ok(())
    }
}

fn node() -> Node { Node::new(NodeConfig::new("Alice", 9001).unwrap(), "alice".into()).unwrap() }

async fn next(events: &Receiver<NodeEvent>) -> NodeEvent { tokio::time::timeout(Duration::from_secs(2), events.recv()).await.unwrap().unwrap() }

#[tokio::test]
async fn loop_handles_discovery_chat_and_shutdown_through_ports() {
    let network = RecordingNetwork::default();
    let (inbound_tx, inbound_rx) = mpsc::channel(4);
    let (command_tx, command_rx) = async_channel::unbounded();
    let (event_tx, event_rx) = async_channel::unbounded();
    let task = tokio::spawn(run_node(node(), network.clone(), inbound_rx, command_rx, event_tx));

    inbound_tx
        .send(Inbound {
            from: "127.0.0.1:50000".parse().unwrap(),
            message: Message::hello("bob", "Bob", 9002),
        })
        .await
        .unwrap();
    assert!(matches!(next(&event_rx).await, NodeEvent::PeerJoined(peer) if peer.id == "bob"));
    command_tx.send(NodeCommand::SendChat(" 안녕 ".into())).await.unwrap();
    assert!(matches!(next(&event_rx).await, NodeEvent::ChatSent { content, sent_to: 1, .. } if content == "안녕"));
    assert_eq!(
        *network.direct.lock().unwrap(),
        vec![("127.0.0.1:9002".parse::<SocketAddr>().unwrap(), Message::chat("alice", "Alice", "안녕"))]
    );
    command_tx.send(NodeCommand::Shutdown).await.unwrap();
    assert_eq!(next(&event_rx).await, NodeEvent::Stopped);
    tokio::time::timeout(Duration::from_secs(2), task).await.unwrap().unwrap();
    assert_eq!(network.broadcasts.lock().unwrap().last(), Some(&Message::goodbye("alice", "Alice")));
}

#[tokio::test]
async fn closed_command_channel_sends_goodbye_and_stops() {
    let network = RecordingNetwork::default();
    let (_inbound_tx, inbound_rx) = mpsc::channel(1);
    let (command_tx, command_rx) = async_channel::unbounded();
    let (event_tx, event_rx) = async_channel::unbounded();
    drop(command_tx);
    tokio::time::timeout(Duration::from_secs(2), run_node(node(), network.clone(), inbound_rx, command_rx, event_tx))
        .await
        .unwrap();
    assert_eq!(next(&event_rx).await, NodeEvent::Stopped);
    assert_eq!(network.broadcasts.lock().unwrap().last(), Some(&Message::goodbye("alice", "Alice")));
}

#[tokio::test]
async fn stalled_peer_does_not_block_inbound_discovery_healthy_sends_or_shutdown() {
    let network = RecordingNetwork {
        stall_port: Some(9002),
        ..Default::default()
    };
    let (inbound_tx, inbound_rx) = mpsc::channel(4);
    let (command_tx, command_rx) = async_channel::unbounded();
    let (event_tx, event_rx) = async_channel::unbounded();
    let task = tokio::spawn(run_node(node(), network.clone(), inbound_rx, command_rx, event_tx));
    for (id, port) in [("bob", 9002), ("carol", 9003)] {
        inbound_tx
            .send(Inbound {
                from: "127.0.0.1:50050".parse().unwrap(),
                message: Message::hello(id, id, port),
            })
            .await
            .unwrap();
        assert!(matches!(next(&event_rx).await, NodeEvent::PeerJoined(_)));
    }
    command_tx.send(NodeCommand::SendChat("hello".into())).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while network.direct.lock().unwrap().len() < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    inbound_tx
        .send(Inbound {
            from: "127.0.0.1:9003".parse().unwrap(),
            message: Message::chat("carol", "carol", "reply"),
        })
        .await
        .unwrap();
    assert!(matches!(next(&event_rx).await, NodeEvent::ChatReceived { content, .. } if content == "reply"));
    inbound_tx
        .send(Inbound {
            from: "127.0.0.1:50050".parse().unwrap(),
            message: Message::hello("dave", "dave", 9004),
        })
        .await
        .unwrap();
    assert!(matches!(next(&event_rx).await, NodeEvent::PeerJoined(peer) if peer.id == "dave"));
    assert!(matches!(
        tokio::time::timeout(SEND_TIMEOUT + Duration::from_secs(1), event_rx.recv())
            .await
            .unwrap()
            .unwrap(),
        NodeEvent::ChatSent {
            sent_to: 1,
            ..
        }
    ));
    assert!(
        network
            .broadcasts
            .lock()
            .unwrap()
            .iter()
            .filter(|message| matches!(message, Message::Hello { .. }))
            .count()
            >= 2
    );
    command_tx.send(NodeCommand::Shutdown).await.unwrap();
    assert_eq!(next(&event_rx).await, NodeEvent::Stopped);
    tokio::time::timeout(Duration::from_secs(1), task).await.unwrap().unwrap();
}

#[tokio::test]
async fn shutdown_reports_cancelled_queued_messages_instead_of_hanging() {
    let network = RecordingNetwork {
        stall_port: Some(9002),
        ..Default::default()
    };
    let (inbound_tx, inbound_rx) = mpsc::channel(1);
    let (command_tx, command_rx) = async_channel::unbounded();
    let (event_tx, event_rx) = async_channel::unbounded();
    let task = tokio::spawn(run_node(node(), network, inbound_rx, command_rx, event_tx));
    inbound_tx
        .send(Inbound {
            from: "127.0.0.1:50050".parse().unwrap(),
            message: Message::hello("bob", "bob", 9002),
        })
        .await
        .unwrap();
    assert!(matches!(next(&event_rx).await, NodeEvent::PeerJoined(_)));
    for _ in 0 .. 3 {
        command_tx.send(NodeCommand::SendChat("pending".into())).await.unwrap();
    }
    command_tx.send(NodeCommand::Shutdown).await.unwrap();
    let mut cancelled = false;
    tokio::time::timeout(SEND_TIMEOUT + Duration::from_secs(1), async {
        loop {
            match event_rx.recv().await.unwrap() {
                | NodeEvent::SendFailed(error) if error.contains("취소") => cancelled = true,
                | NodeEvent::Stopped => break,
                | _ => {},
            }
        }
    })
    .await
    .unwrap();
    assert!(cancelled);
    task.await.unwrap();
}

#[tokio::test]
async fn sending_reports_partial_success_and_total_failure() {
    let mut node = node();
    for (id, port) in [("bob", 9002), ("carol", 9003)] {
        node.receive(
            Inbound {
                from: "127.0.0.1:50000".parse().unwrap(),
                message: Message::hello(id, id, port),
            },
            Instant::now(),
            SystemTime::UNIX_EPOCH,
        );
    }
    let network = RecordingNetwork {
        fail_port: Some(9002),
        ..Default::default()
    };
    let network = Arc::new(network);
    assert!(matches!(
        send_chat(node.prepare_chat("hi").unwrap(), network.clone()).await,
        NodeEvent::ChatSent {
            sent_to: 1,
            ..
        }
    ));
    assert_eq!(network.direct.lock().unwrap().len(), 2);

    node.receive(
        Inbound {
            from: "127.0.0.1:50000".parse().unwrap(),
            message: Message::goodbye("carol", "carol"),
        },
        Instant::now(),
        SystemTime::UNIX_EPOCH,
    );
    assert_eq!(
        send_chat(node.prepare_chat("hi").unwrap(), network.clone()).await,
        NodeEvent::SendFailed("offline".into())
    );
    assert_eq!(node.prepare_chat("  ").unwrap_err(), "빈 메시지는 보낼 수 없습니다.");
    assert_eq!(network.direct.lock().unwrap().len(), 3);
}
