//! 순수한 애플리케이션 유스케이스와 경계 계약.
//! 소켓, 스레드, 채널, 현재 시각, 난수 생성은 바깥 계층이 담당한다.

mod config;
mod events;
mod node;
mod ports;

pub use config::{ConfigError,
                 MAX_NICKNAME_CHARS,
                 NodeConfig};
pub use events::{LeaveReason,
                 NodeCommand,
                 NodeEvent,
                 PeerInfo};
pub use node::{ChatDispatch,
               Node,
               PEER_TIMEOUT};
pub use ports::{Inbound,
                NetworkPort};

#[cfg(test)]
mod tests;
