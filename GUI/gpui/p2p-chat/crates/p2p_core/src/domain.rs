use crate::P2PError;
use serde::{Deserialize,
            Serialize};
use std::{collections::HashMap,
          net::SocketAddr,
          time::{Duration,
                 Instant,
                 SystemTime}};

/// UDP 데이터그램 한 개의 최대 크기. 수신 버퍼 크기와 같아야 한다.
pub const MAX_DATAGRAM_BYTES: usize = 4096;

/// 피어 간 주고받는 P2P 프로토콜 메시지 규약
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum Message {
    /// 피어 검색 및 닉네임 알림 브로드캐스트
    #[serde(rename = "HELLO")]
    Hello { node_id: String, nickname: String, listen_port: u16 },
    /// 피어 간 직접 메시지 전송 (1:1 또는 멀티캐스트)
    #[serde(rename = "CHAT")]
    Chat { node_id: String, nickname: String, content: String },
    /// 정상 종료 알림
    #[serde(rename = "GOODBYE")]
    Goodbye { node_id: String, nickname: String },
}

impl Message {
    pub fn hello(node_id: impl Into<String>, nickname: impl Into<String>, listen_port: u16) -> Self {
        Self::Hello {
            node_id: node_id.into(),
            nickname: nickname.into(),
            listen_port,
        }
    }

    pub fn chat(node_id: impl Into<String>, nickname: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Chat {
            node_id: node_id.into(),
            nickname: nickname.into(),
            content: content.into(),
        }
    }

    pub fn goodbye(node_id: impl Into<String>, nickname: impl Into<String>) -> Self {
        Self::Goodbye {
            node_id: node_id.into(),
            nickname: nickname.into(),
        }
    }

    /// 와이어 포맷(JSON 바이트)으로 직렬화한다. 데이터그램 한도를 넘으면
    /// 실패한다.
    pub fn encode(&self) -> Result<Vec<u8>, P2PError> {
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > MAX_DATAGRAM_BYTES {
            return Err(P2PError::MessageTooLarge {
                size: bytes.len(),
                max: MAX_DATAGRAM_BYTES,
            });
        }
        Ok(bytes)
    }

    /// 수신한 바이트를 메시지로 역직렬화한다.
    pub fn decode(bytes: &[u8]) -> Result<Self, P2PError> { Ok(serde_json::from_slice(bytes)?) }
}

/// 대화 기록에 찍히는 발신자
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Speaker {
    /// 내가 보낸 메시지
    Me,
    /// 피어가 보낸 메시지
    Peer { id: String, nickname: String },
}

/// 대화 기록 한 줄: 주고받은 메시지 또는 입장/퇴장 같은 시스템 공지
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatEntry {
    Message { speaker: Speaker, content: String, at: SystemTime },
    Notice { text: String, at: SystemTime },
}

/// 네트워크 상에서 발견된 피어 정보
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub id: String,
    pub nickname: String,
    pub endpoint: SocketAddr,
    pub last_seen: Instant,
}

impl Peer {
    pub fn new(id: String, nickname: String, endpoint: SocketAddr, now: Instant) -> Self {
        Self {
            id,
            nickname,
            endpoint,
            last_seen: now,
        }
    }
}

/// 도메인 이벤트 (순수 비즈니스 로직 결과)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEvent {
    Discovered(Peer),
    Updated(Peer),
    Removed { id: String, nickname: String },
    Expired { id: String, nickname: String },
}

/// 순수 인메모리 피어 테이블 (I/O, 소켓 없이 오직 상태 전이만 담당)
#[derive(Debug, Default)]
pub struct PeerTable {
    peers: HashMap<String, Peer>,
}

impl PeerTable {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    /// HELLO 메시지 수신 시 피어 등록/갱신
    pub fn register_or_update(&mut self, id: String, nickname: String, endpoint: SocketAddr, now: Instant) -> PeerEvent {
        if let Some(peer) = self.peers.get_mut(&id) {
            peer.nickname = nickname.clone();
            peer.endpoint = endpoint;
            peer.last_seen = now;
            PeerEvent::Updated(peer.clone())
        } else {
            let new_peer = Peer::new(id.clone(), nickname, endpoint, now);
            self.peers.insert(id, new_peer.clone());
            PeerEvent::Discovered(new_peer)
        }
    }

    /// GOODBYE 수신 시 피어 명시적 제거
    pub fn remove(&mut self, id: &str) -> Option<PeerEvent> {
        self.peers.remove(id).map(|p| PeerEvent::Removed {
            id: p.id,
            nickname: p.nickname,
        })
    }

    /// 타임아웃된 피어 일괄 제거 (순수 함수)
    pub fn cleanup_expired(&mut self, now: Instant, timeout: Duration) -> Vec<PeerEvent> {
        let mut expired_keys = Vec::new();
        for (id, peer) in &self.peers {
            if now.saturating_duration_since(peer.last_seen) > timeout {
                expired_keys.push((id.clone(), peer.nickname.clone()));
            }
        }

        let mut events = Vec::new();
        for (id, nickname) in expired_keys {
            self.peers.remove(&id);
            events.push(PeerEvent::Expired {
                id,
                nickname,
            });
        }
        events
    }

    pub fn get_all(&self) -> Vec<&Peer> { self.peers.values().collect() }

    pub fn count(&self) -> usize { self.peers.len() }
}

#[cfg(test)]
#[path = "domain_tests.rs"]
mod tests;
