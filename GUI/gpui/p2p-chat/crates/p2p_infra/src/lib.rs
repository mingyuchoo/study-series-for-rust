//! 애플리케이션의 NetworkPort를 UDP 발견과 인증된 QUIC 전송으로 구현한다.
//! 프로토콜 변환은 protocol, 네트워크 I/O는 discovery와 quic에 격리한다.

mod discovery;
mod identity;
mod protocol;
mod quic;

use async_trait::async_trait;
pub use discovery::{MULTICAST_IPV4,
                    MULTICAST_PORT};
use identity::{Identity,
               TrustStore};
use p2p_app::{Inbound,
              NetworkPort};
use p2p_core::{Message,
               MessageError};
pub use quic::SEND_TIMEOUT;
use quic::{Peers,
           accept_connections,
           server_config};
use quinn::Endpoint;
use std::{collections::HashMap,
          io,
          net::{Ipv4Addr,
                SocketAddr,
                SocketAddrV4},
          path::Path,
          sync::{Arc,
                 Mutex}};
use thiserror::Error;
use tokio::{net::UdpSocket,
            sync::mpsc,
            task::JoinHandle};

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Message(#[from] MessageError),
    #[error("QUIC error: {0}")]
    Quic(String),
}

/// UDP 발견을 유지하고, 인증서를 검증하는 상호 TLS 연결로 채팅을 전송한다.
pub struct QuicNetworkAdapter {
    endpoint: Endpoint,
    discovery_socket: Arc<UdpSocket>,
    identity: Arc<Identity>,
    peers: Arc<Mutex<Peers>>,
    tasks: Vec<JoinHandle<()>>,
}

impl QuicNetworkAdapter {
    /// 상태 디렉터리 아래 포트별로 키와 인증서 지문을 저장한다.
    pub async fn bind(listen_port: u16, state_directory: &Path) -> io::Result<(Self, mpsc::Receiver<Inbound>)> {
        let socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, listen_port))?;
        socket.set_nonblocking(true)?;
        let directory = state_directory.join(socket.local_addr()?.port().to_string());
        let identity = Arc::new(Identity::load(&directory)?);
        let peers = Arc::new(Mutex::new(Peers {
            discovered: HashMap::new(),
            trust: TrustStore::load(&directory)?,
        }));
        let server = server_config(&identity, &peers.lock().unwrap())?;
        let endpoint = Endpoint::new(quinn::EndpointConfig::default(), Some(server), socket, Arc::new(quinn::TokioRuntime))?;
        let discovery_socket = Arc::new(discovery::bind_multicast()?);
        let (tx, rx) = mpsc::channel(256);
        let tasks = vec![
            tokio::spawn(discovery::discover(
                discovery_socket.clone(),
                endpoint.clone(),
                identity.clone(),
                peers.clone(),
                tx.clone(),
            )),
            tokio::spawn(accept_connections(endpoint.clone(), peers.clone(), tx)),
        ];
        Ok((
            Self {
                endpoint,
                discovery_socket,
                identity,
                peers,
                tasks,
            },
            rx,
        ))
    }

    pub fn node_id(&self) -> &str { &self.identity.node_id }

    pub fn listen_port(&self) -> u16 { self.endpoint.local_addr().expect("bound endpoint").port() }
}

impl Drop for QuicNetworkAdapter {
    fn drop(&mut self) {
        self.endpoint.close(0u32.into(), b"shutdown");
        for task in &self.tasks {
            task.abort();
        }
    }
}

#[async_trait]
impl NetworkPort for QuicNetworkAdapter {
    type Error = NetworkError;

    async fn broadcast_discovery(&self, message: &Message) -> Result<(), NetworkError> {
        let payload = protocol::encode_discovery(message, self.identity.certificate.as_ref(), self.node_id())?;
        self.discovery_socket
            .send_to(&payload, SocketAddrV4::new(MULTICAST_IPV4, MULTICAST_PORT))
            .await?;
        Ok(())
    }

    async fn send_direct(&self, target: SocketAddr, message: &Message) -> Result<(), NetworkError> {
        if !matches!(message, Message::Chat { node_id, .. } if node_id == self.node_id()) {
            return Err(NetworkError::Quic("invalid outgoing chat identity".into()));
        }
        quic::send_chat(&self.endpoint, &self.identity, &self.peers, target, &message.encode()?).await
    }
}

#[cfg(test)]
mod tests;
