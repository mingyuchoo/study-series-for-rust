//! 애플리케이션 계층: 네트워크 어댑터와 도메인 모델을 조립해 P2P 노드를
//! 실행한다. UI는 [`start`]가 돌려주는 채널([`NodeCommand`] 전송, [`NodeEvent`]
//! 수신)만 사용한다.

use async_channel::{Receiver,
                    Sender};
use p2p_core::{Message,
               NetworkPort,
               P2PError,
               Peer,
               PeerEvent,
               PeerTable};
use p2p_infra::{Inbound,
                UdpNetworkAdapter};
use std::{net::SocketAddr,
          thread::JoinHandle,
          time::{Duration,
                 Instant,
                 SystemTime}};
use tokio::sync::mpsc;
use uuid::Uuid;

const HELLO_INTERVAL: Duration = Duration::from_secs(2);
const SWEEP_INTERVAL: Duration = Duration::from_secs(3);
const PEER_TIMEOUT: Duration = Duration::from_secs(6);

/// 노드 실행 설정
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeConfig {
    pub nickname: String,
    pub listen_port: u16,
}

/// UI → 노드 명령
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeCommand {
    /// 연결된 모든 피어에게 채팅 메시지 전송
    SendChat(String),
    /// GOODBYE를 보내고 종료
    Shutdown,
}

/// 피어 정보 스냅샷
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    pub id: String,
    pub nickname: String,
    pub endpoint: SocketAddr,
}

impl From<&Peer> for PeerInfo {
    fn from(peer: &Peer) -> Self {
        Self {
            id: peer.id.clone(),
            nickname: peer.nickname.clone(),
            endpoint: peer.endpoint,
        }
    }
}

/// 피어가 목록에서 빠진 이유
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveReason {
    /// GOODBYE 수신 (정상 종료)
    Goodbye,
    /// 응답 없음 (타임아웃)
    Timeout,
}

/// 노드 → UI 이벤트
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeEvent {
    PeerJoined(PeerInfo),
    PeerUpdated(PeerInfo),
    PeerLeft {
        id: String,
        nickname: String,
        reason: LeaveReason,
    },
    ChatReceived {
        id: String,
        nickname: String,
        content: String,
        at: SystemTime,
    },
    ChatSent {
        content: String,
        sent_to: usize,
        at: SystemTime,
    },
    SendFailed(String),
    /// 노드 루프가 끝났다 (GOODBYE 전송 완료)
    Stopped,
}

/// 실행 중인 노드. `commands`로 명령을 보내고 `events`에서 이벤트를 받는다.
pub struct NodeSession {
    pub node_id: String,
    pub commands: Sender<NodeCommand>,
    pub events: Receiver<NodeEvent>,
    thread: JoinHandle<()>,
}

impl NodeSession {
    /// 종료 명령을 보내고, 노드가 GOODBYE를 보내고 끝날 때까지 기다린다.
    pub fn shutdown(self) {
        let _ = self.commands.try_send(NodeCommand::Shutdown);
        let _ = self.thread.join();
    }
}

/// 노드를 전용 스레드(자체 tokio 런타임)에서 시작한다.
/// 소켓 바인딩에 성공하면 세션을 돌려주고, 실패하면 그 오류를 돌려준다.
pub fn start(config: NodeConfig) -> Result<NodeSession, P2PError> {
    let node_id = Uuid::new_v4().to_string();
    let (command_tx, command_rx) = async_channel::unbounded();
    let (event_tx, event_rx) = async_channel::unbounded();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), P2PError>>();

    let thread_node_id = node_id.clone();
    let thread = std::thread::Builder::new().name("p2p-node".into()).spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            | Ok(runtime) => runtime,
            | Err(e) => {
                let _ = ready_tx.send(Err(P2PError::Io(e)));
                return;
            },
        };
        runtime.block_on(async move {
            match UdpNetworkAdapter::bind(config.listen_port).await {
                | Ok((adapter, inbound)) => {
                    let _ = ready_tx.send(Ok(()));
                    run_node(config, thread_node_id, adapter, inbound, command_rx, event_tx).await;
                },
                | Err(e) => {
                    let _ = ready_tx.send(Err(e));
                },
            }
        });
    })?;

    match ready_rx.recv() {
        | Ok(Ok(())) => Ok(NodeSession {
            node_id,
            commands: command_tx,
            events: event_rx,
            thread,
        }),
        | Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        },
        | Err(_) => {
            let _ = thread.join();
            Err(P2PError::Network("노드 스레드가 시작되지 않았습니다.".into()))
        },
    }
}

/// 노드 메인 루프. 주기적 HELLO, 수신 메시지 처리, 타임아웃 정리, UI 명령
/// 처리를 한 태스크에서 순서대로 수행한다. 피어 테이블을 이 태스크만 소유하므로
/// Mutex가 필요 없다.
async fn run_node<N: NetworkPort>(
    config: NodeConfig,
    node_id: String,
    network: N,
    mut inbound: mpsc::Receiver<Inbound>,
    commands: Receiver<NodeCommand>,
    events: Sender<NodeEvent>,
) {
    let mut peers = PeerTable::new();
    let mut hello_tick = tokio::time::interval(HELLO_INTERVAL);
    let mut sweep_tick = tokio::time::interval(SWEEP_INTERVAL);

    loop {
        tokio::select! {
            _ = hello_tick.tick() => {
                let hello = Message::hello(&node_id, &config.nickname, config.listen_port);
                let _ = network.broadcast_discovery(&hello).await;
            },
            _ = sweep_tick.tick() => {
                for event in peers.cleanup_expired(Instant::now(), PEER_TIMEOUT) {
                    if let PeerEvent::Expired { id, nickname } = event {
                        emit(&events, NodeEvent::PeerLeft { id, nickname, reason: LeaveReason::Timeout });
                    }
                }
            },
            received = inbound.recv() => match received {
                Some(inbound) => handle_inbound(&mut peers, &node_id, inbound, &events),
                None => break,
            },
            command = commands.recv() => match command {
                Ok(NodeCommand::SendChat(text)) => {
                    send_chat(&network, &peers, &node_id, &config.nickname, &text, &events).await;
                },
                Ok(NodeCommand::Shutdown) | Err(_) => break,
            },
        }
    }

    let goodbye = Message::goodbye(&node_id, &config.nickname);
    let _ = network.broadcast_discovery(&goodbye).await;
    emit(&events, NodeEvent::Stopped);
}

/// 수신 메시지를 피어 상태에 반영하고, 변화가 있으면 UI 이벤트로 알린다.
fn handle_inbound(peers: &mut PeerTable, self_id: &str, inbound: Inbound, events: &Sender<NodeEvent>) {
    let Inbound {
        from,
        message,
    } = inbound;
    match message {
        | Message::Hello {
            node_id,
            nickname,
            listen_port,
        } if node_id != self_id => {
            // 대화 포트는 메시지에 담긴 listen_port를 쓰고, 주소는 UDP 발신지의
            // IP를 쓴다.
            let endpoint = SocketAddr::new(from.ip(), listen_port);
            match peers.register_or_update(node_id, nickname, endpoint, Instant::now()) {
                | PeerEvent::Discovered(peer) => emit(events, NodeEvent::PeerJoined(PeerInfo::from(&peer))),
                | PeerEvent::Updated(peer) => emit(events, NodeEvent::PeerUpdated(PeerInfo::from(&peer))),
                | _ => {},
            }
        },
        | Message::Goodbye {
            node_id, ..
        } => {
            if let Some(PeerEvent::Removed {
                id,
                nickname,
            }) = peers.remove(&node_id)
            {
                emit(
                    events,
                    NodeEvent::PeerLeft {
                        id,
                        nickname,
                        reason: LeaveReason::Goodbye,
                    },
                );
            }
        },
        | Message::Chat {
            node_id,
            nickname,
            content,
        } if node_id != self_id => {
            emit(
                events,
                NodeEvent::ChatReceived {
                    id: node_id,
                    nickname,
                    content,
                    at: SystemTime::now(),
                },
            );
        },
        | _ => {},
    }
}

/// 채팅 메시지를 모든 피어에게 직접 보낸다. 결과는 ChatSent 또는 SendFailed
/// 이벤트로 알린다.
async fn send_chat<N: NetworkPort>(network: &N, peers: &PeerTable, self_id: &str, nickname: &str, text: &str, events: &Sender<NodeEvent>) {
    let content = text.trim();
    if content.is_empty() {
        emit(events, NodeEvent::SendFailed("빈 메시지는 보낼 수 없습니다.".into()));
        return;
    }

    let message = Message::chat(self_id, nickname, content);
    // 크기 초과는 피어 수와 무관하므로 전송 전에 한 번만 검사한다.
    if let Err(e) = message.encode() {
        emit(events, NodeEvent::SendFailed(e.to_string()));
        return;
    }

    let targets: Vec<SocketAddr> = peers.get_all().into_iter().map(|p| p.endpoint).collect();
    if targets.is_empty() {
        emit(events, NodeEvent::SendFailed("연결된 피어가 없습니다.".into()));
        return;
    }

    let mut sent_to = 0;
    let mut last_error = String::new();
    for target in targets {
        match network.send_direct(target, &message).await {
            | Ok(()) => sent_to += 1,
            | Err(e) => last_error = e.to_string(),
        }
    }

    if sent_to == 0 {
        emit(events, NodeEvent::SendFailed(last_error));
    } else {
        emit(
            events,
            NodeEvent::ChatSent {
                content: content.to_string(),
                sent_to,
                at: SystemTime::now(),
            },
        );
    }
}

/// 이벤트 채널은 무제한이므로 전송 실패는 UI가 이미 닫혔다는 뜻이다.
fn emit(events: &Sender<NodeEvent>, event: NodeEvent) { let _ = events.try_send(event); }
