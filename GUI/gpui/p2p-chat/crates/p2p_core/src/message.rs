//! 피어 메시지 규약과 순수한 JSON 변환. 소켓이나 런타임에 의존하지 않는다.

use serde::{Deserialize,
            Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MessageError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Message too large: {size} bytes (max {max})")]
    MessageTooLarge { size: usize, max: usize },
}

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
    pub fn encode(&self) -> Result<Vec<u8>, MessageError> {
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > MAX_DATAGRAM_BYTES {
            return Err(MessageError::MessageTooLarge {
                size: bytes.len(),
                max: MAX_DATAGRAM_BYTES,
            });
        }
        Ok(bytes)
    }

    /// 수신한 바이트를 메시지로 역직렬화한다.
    pub fn decode(bytes: &[u8]) -> Result<Self, MessageError> { Ok(serde_json::from_slice(bytes)?) }
}

#[cfg(test)]
#[path = "message_tests.rs"]
mod tests;
