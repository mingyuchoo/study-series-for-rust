//! 화면 상태의 순수한 전이. GPUI, 현재 시각, 스크롤 갱신에 의존하지 않는다.

use p2p_app::{LeaveReason,
              NodeEvent,
              PeerInfo};
use p2p_core::{ChatEntry,
               Speaker};
use std::time::SystemTime;

#[derive(Default)]
pub struct Conversation {
    pub peers: Vec<PeerInfo>,
    pub entries: Vec<ChatEntry>,
    pub error: Option<String>,
    pub stopped: bool,
}

impl Conversation {
    /// 새 대화 항목을 추가했으면 true를 반환한다.
    pub fn apply(&mut self, event: NodeEvent, at: SystemTime) -> bool {
        let mut appended = true;
        match event {
            | NodeEvent::PeerJoined(peer) => {
                self.entries.push(notice(format!("'{}' 입장", peer.nickname), at));
                self.peers.push(peer);
            },
            | NodeEvent::PeerUpdated(peer) => {
                appended = false;
                match self.peers.iter_mut().find(|p| p.id == peer.id) {
                    | Some(existing) => *existing = peer,
                    | None => self.peers.push(peer),
                }
            },
            | NodeEvent::PeerLeft {
                id,
                nickname,
                reason,
            } => {
                self.peers.retain(|p| p.id != id);
                let text = match reason {
                    | LeaveReason::Goodbye => format!("'{nickname}' 퇴장"),
                    | LeaveReason::Timeout => format!("'{nickname}' 응답 없음 (연결 끊김)"),
                };
                self.entries.push(notice(text, at));
            },
            | NodeEvent::ChatReceived {
                id,
                nickname,
                content,
                at,
            } => {
                self.entries.push(ChatEntry::Message {
                    speaker: Speaker::Peer {
                        id,
                        nickname,
                    },
                    content,
                    at,
                });
            },
            | NodeEvent::ChatSent {
                content,
                at,
                ..
            } => {
                self.entries.push(ChatEntry::Message {
                    speaker: Speaker::Me,
                    content,
                    at,
                });
            },
            | NodeEvent::SendFailed(reason) => {
                appended = false;
                self.error = Some(reason);
            },
            | NodeEvent::Stopped => {
                appended = false;
                self.stopped = true;
            },
        }
        appended
    }
}

fn notice(text: String, at: SystemTime) -> ChatEntry {
    ChatEntry::Notice {
        text,
        at,
    }
}

#[cfg(test)]
mod tests;
