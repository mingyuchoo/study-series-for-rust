use p2p_core::Peer;
use std::{net::SocketAddr,
          time::SystemTime};

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
