use async_trait::async_trait;
use p2p_core::Message;
use std::{error::Error,
          net::SocketAddr};

/// 소켓과 수신 채널을 노출하지 않는 애플리케이션 입력.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inbound {
    pub from: SocketAddr,
    pub message: Message,
}

/// 전송 계약만 정의한다. 구체적인 I/O 오류와 구현은 바깥 계층이 소유한다.
#[async_trait]
pub trait NetworkPort: Send + Sync {
    type Error: Error + Send + Sync;

    async fn broadcast_discovery(&self, message: &Message) -> Result<(), Self::Error>;
    async fn send_direct(&self, target: SocketAddr, message: &Message) -> Result<(), Self::Error>;
}
