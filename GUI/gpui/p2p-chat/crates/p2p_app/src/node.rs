use crate::{ConfigError,
            Inbound,
            LeaveReason,
            NodeConfig,
            NodeEvent,
            PeerInfo};
use p2p_core::{Message,
               PeerEvent,
               PeerTable};
use std::{net::SocketAddr,
          time::{Duration,
                 Instant,
                 SystemTime}};

pub const PEER_TIMEOUT: Duration = Duration::from_secs(6);

/// 피어 상태와 채팅 규칙. 시각과 노드 ID는 외부에서 주입한다.
pub struct Node {
    config: NodeConfig,
    node_id: String,
    peers: PeerTable,
}

impl Node {
    pub fn new(config: NodeConfig, node_id: String) -> Result<Self, ConfigError> {
        Ok(Self {
            config: NodeConfig::new(config.nickname, config.listen_port)?,
            node_id,
            peers: PeerTable::new(),
        })
    }

    pub fn hello(&self) -> Message { Message::hello(&self.node_id, &self.config.nickname, self.config.listen_port) }

    pub fn goodbye(&self) -> Message { Message::goodbye(&self.node_id, &self.config.nickname) }

    pub fn receive(&mut self, inbound: Inbound, now: Instant, at: SystemTime) -> Option<NodeEvent> {
        match inbound.message {
            | Message::Hello {
                node_id,
                nickname,
                listen_port,
            } if node_id != self.node_id => {
                let endpoint = SocketAddr::new(inbound.from.ip(), listen_port);
                match self.peers.register_or_update(node_id, nickname, endpoint, now) {
                    | PeerEvent::Discovered(peer) => Some(NodeEvent::PeerJoined(PeerInfo::from(&peer))),
                    | PeerEvent::Updated(peer) => Some(NodeEvent::PeerUpdated(PeerInfo::from(&peer))),
                    | _ => None,
                }
            },
            | Message::Goodbye {
                node_id, ..
            } => match self.peers.remove(&node_id) {
                | Some(PeerEvent::Removed {
                    id,
                    nickname,
                }) => Some(NodeEvent::PeerLeft {
                    id,
                    nickname,
                    reason: LeaveReason::Goodbye,
                }),
                | _ => None,
            },
            | Message::Chat {
                node_id,
                nickname,
                content,
            } if node_id != self.node_id => Some(NodeEvent::ChatReceived {
                id: node_id,
                nickname,
                content,
                at,
            }),
            | _ => None,
        }
    }

    pub fn expire(&mut self, now: Instant) -> Vec<NodeEvent> {
        self.peers
            .cleanup_expired(now, PEER_TIMEOUT)
            .into_iter()
            .filter_map(|event| match event {
                | PeerEvent::Expired {
                    id,
                    nickname,
                } => Some(NodeEvent::PeerLeft {
                    id,
                    nickname,
                    reason: LeaveReason::Timeout,
                }),
                | _ => None,
            })
            .collect()
    }

    /// 전송 내용과 대상만 결정한다. 실제 네트워크 호출은 하지 않는다.
    pub fn prepare_chat(&self, text: &str) -> Result<ChatDispatch, String> {
        let content = text.trim();
        if content.is_empty() {
            return Err("빈 메시지는 보낼 수 없습니다.".into());
        }
        let message = Message::chat(&self.node_id, &self.config.nickname, content);
        // 크기 제한은 피어 수와 무관하므로 대상 검사 전에 검증한다.
        message.encode().map_err(|error| error.to_string())?;
        let targets: Vec<SocketAddr> = self.peers.get_all().into_iter().map(|peer| peer.endpoint).collect();
        if targets.is_empty() {
            return Err("연결된 피어가 없습니다.".into());
        }
        Ok(ChatDispatch {
            message,
            targets,
        })
    }
}

/// 검증된 전송 계획. 결과를 주입하면 화면에 전달할 이벤트를 만든다.
#[derive(Debug)]
pub struct ChatDispatch {
    message: Message,
    targets: Vec<SocketAddr>,
}

impl ChatDispatch {
    pub fn message(&self) -> &Message { &self.message }

    pub fn targets(&self) -> &[SocketAddr] { &self.targets }

    pub fn finish(self, sent_to: usize, last_error: String, at: SystemTime) -> NodeEvent {
        if sent_to == 0 {
            return NodeEvent::SendFailed(last_error);
        }
        let Message::Chat {
            content, ..
        } = self.message
        else {
            unreachable!("전송 계획은 prepare_chat에서 CHAT 메시지로만 생성한다");
        };
        NodeEvent::ChatSent {
            content,
            sent_to,
            at,
        }
    }
}
