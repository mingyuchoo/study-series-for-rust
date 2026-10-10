pub mod domain;

use async_trait::async_trait;
pub use domain::{ChatEntry,
                 MAX_DATAGRAM_BYTES,
                 Message,
                 Peer,
                 PeerEvent,
                 PeerTable,
                 Speaker};
use std::net::SocketAddr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum P2PError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Message too large: {size} bytes (max {max})")]
    MessageTooLarge { size: usize, max: usize },
}

/// 네트워크 전송 포트 (Clean Architecture의 Outbound Port 인터페이스)
#[async_trait]
pub trait NetworkPort: Send + Sync {
    /// 멀티캐스트 그룹에 디스커버리 메시지 브로드캐스트
    async fn broadcast_discovery(&self, message: &Message) -> Result<(), P2PError>;
    /// 특정 피어에게 직접 메시지 전송
    async fn send_direct(&self, target: SocketAddr, message: &Message) -> Result<(), P2PError>;
}
