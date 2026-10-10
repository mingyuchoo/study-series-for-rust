//! 순수한 도메인 상태와 메시지 변환. 시간은 호출자가 전달한다.

pub mod domain;
pub mod message;

pub use domain::{ChatEntry,
                 Peer,
                 PeerEvent,
                 PeerTable,
                 Speaker};
pub use message::{MAX_DATAGRAM_BYTES,
                  Message,
                  MessageError};
