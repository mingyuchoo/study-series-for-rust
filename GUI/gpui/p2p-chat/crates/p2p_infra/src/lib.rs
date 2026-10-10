//! UDP 멀티캐스트 발견과 인증된 QUIC 채팅을 구현한다.

mod identity;

use async_trait::async_trait;
use identity::{Identity,
               TrustStore,
               fingerprint};
use p2p_app::{Inbound,
              NetworkPort};
use p2p_core::{MAX_DATAGRAM_BYTES,
               Message,
               MessageError};
use quinn::{Connection,
            Endpoint,
            RecvStream,
            SendStream,
            crypto::rustls::{QuicClientConfig,
                             QuicServerConfig}};
use rustls::{RootCertStore,
             pki_types::CertificateDer};
use serde::{Deserialize,
            Serialize};
use socket2::{Domain,
              Protocol,
              Socket,
              Type};
use std::{collections::HashMap,
          io,
          net::{Ipv4Addr,
                SocketAddr,
                SocketAddrV4},
          path::Path,
          sync::{Arc,
                 Mutex},
          time::{Duration,
                 Instant}};
use thiserror::Error;
use tokio::{net::UdpSocket,
            sync::{Mutex as AsyncMutex,
                   mpsc},
            task::{JoinHandle,
                   JoinSet}};

pub const MULTICAST_IPV4: Ipv4Addr = Ipv4Addr::new(239, 255, 42, 99);
pub const MULTICAST_PORT: u16 = 50050;
pub const SEND_TIMEOUT: Duration = Duration::from_secs(3);
const SERVER_NAME: &str = "p2p-chat";
const ALPN: &[u8] = b"p2p-chat/1";
const MAX_PEERS: usize = 256;
const MAX_CONNECTIONS: usize = 64;
const PEER_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Message(#[from] MessageError),
    #[error("QUIC error: {0}")]
    Quic(String),
}

#[derive(Serialize, Deserialize)]
struct Discovery {
    protocol: u8,
    certificate: Vec<u8>,
    #[serde(flatten)]
    message: Message,
}

struct Link {
    connection: Connection,
    send: SendStream,
    recv: RecvStream,
}

impl Drop for Link {
    fn drop(&mut self) { self.connection.close(0u32.into(), b"chat stream ended"); }
}

struct Peer {
    node_id: String,
    certificate: CertificateDer<'static>,
    seen: Instant,
    link: Arc<AsyncMutex<Option<Link>>>,
}

struct Peers {
    discovered: HashMap<SocketAddr, Peer>,
    trust: TrustStore,
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
        let discovery_socket = Arc::new(bind_multicast()?);
        let (tx, rx) = mpsc::channel(256);
        let tasks = vec![
            tokio::spawn(discover(
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

    async fn send_chat(&self, target: SocketAddr, payload: &[u8]) -> Result<(), NetworkError> {
        let (certificate, link) = {
            let peers = self.peers.lock().unwrap();
            let peer = peers
                .discovered
                .get(&target)
                .filter(|peer| peer.seen.elapsed() <= PEER_TIMEOUT)
                .ok_or_else(|| NetworkError::Quic("peer has not been discovered".into()))?;
            (peer.certificate.clone(), peer.link.clone())
        };
        tokio::time::timeout(SEND_TIMEOUT, async {
            let mut cached = link.lock().await;
            // 캐시에서 꺼내 소유해야 전송 작업 취소 시에도 미완료 스트림이
            // 닫힌다.
            let mut active = match cached.take().filter(|link| link.connection.close_reason().is_none()) {
                | Some(link) => link,
                | None => {
                    let client = client_config(&self.identity, certificate)?;
                    let connection = self
                        .endpoint
                        .connect_with(client, target, SERVER_NAME)
                        .map_err(quic_error)?
                        .await
                        .map_err(quic_error)?;
                    authenticate(&connection, &self.peers)?;
                    let (send, recv) = connection.open_bi().await.map_err(quic_error)?;
                    Link {
                        connection,
                        send,
                        recv,
                    }
                },
            };
            active.send.write_all(&(payload.len() as u32).to_be_bytes()).await.map_err(quic_error)?;
            active.send.write_all(payload).await.map_err(quic_error)?;
            let mut ack = [0];
            active.recv.read_exact(&mut ack).await.map_err(quic_error)?;
            if ack != [1] {
                return Err(NetworkError::Quic("invalid chat acknowledgement".into()));
            }
            *cached = Some(active);
            Ok(())
        })
        .await
        .unwrap_or_else(|_| Err(NetworkError::Quic(format!("send to {target} timed out"))))
    }
}

impl Drop for QuicNetworkAdapter {
    fn drop(&mut self) {
        self.endpoint.close(0u32.into(), b"shutdown");
        for task in &self.tasks {
            task.abort();
        }
    }
}

fn quic_error(error: impl std::fmt::Display) -> NetworkError { NetworkError::Quic(error.to_string()) }

fn server_config(identity: &Identity, peers: &Peers) -> io::Result<quinn::ServerConfig> {
    let mut roots = RootCertStore::empty();
    roots.add(identity.certificate.clone()).map_err(io::Error::other)?;
    for peer in peers.discovered.values() {
        roots.add(peer.certificate.clone()).map_err(io::Error::other)?;
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider.clone())
        .build()
        .map_err(io::Error::other)?;
    let mut tls = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(io::Error::other)?
        .with_client_cert_verifier(verifier)
        .with_single_cert(vec![identity.certificate.clone()], identity.key.clone_key())
        .map_err(io::Error::other)?;
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let mut config = quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(tls).map_err(io::Error::other)?));
    let transport = Arc::get_mut(&mut config.transport).unwrap();
    transport.max_concurrent_bidi_streams(1u32.into());
    transport.max_concurrent_uni_streams(0u32.into());
    transport.max_idle_timeout(Some(Duration::from_secs(10).try_into().unwrap()));
    transport.keep_alive_interval(Some(Duration::from_secs(2)));
    Ok(config)
}

fn client_config(identity: &Identity, certificate: CertificateDer<'static>) -> io::Result<quinn::ClientConfig> {
    let mut roots = RootCertStore::empty();
    roots.add(certificate).map_err(io::Error::other)?;
    let mut tls = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(io::Error::other)?
        .with_root_certificates(roots)
        .with_client_auth_cert(vec![identity.certificate.clone()], identity.key.clone_key())
        .map_err(io::Error::other)?;
    tls.alpn_protocols = vec![ALPN.to_vec()];
    Ok(quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(tls).map_err(io::Error::other)?)))
}

fn authenticate(connection: &Connection, peers: &Mutex<Peers>) -> Result<String, NetworkError> {
    let identity = connection
        .peer_identity()
        .ok_or_else(|| NetworkError::Quic("missing peer certificate".into()))?;
    let certificates = identity
        .downcast::<Vec<CertificateDer<'static>>>()
        .map_err(|_| NetworkError::Quic("invalid peer identity".into()))?;
    let certificate = certificates.first().ok_or_else(|| NetworkError::Quic("missing peer certificate".into()))?;
    let node_id = fingerprint(certificate);
    let address = connection.remote_address();
    let mut peers = peers.lock().unwrap();
    let peer = peers
        .discovered
        .get(&address)
        .filter(|peer| peer.node_id == node_id && peer.certificate == *certificate && peer.seen.elapsed() <= PEER_TIMEOUT)
        .ok_or_else(|| NetworkError::Quic("certificate does not match discovered peer".into()))?;
    let node_id = peer.node_id.clone();
    peers.trust.pin(address, &node_id)?;
    Ok(node_id)
}

fn bind_multicast() -> io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(not(windows))]
    socket.set_reuse_port(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, MULTICAST_PORT).into())?;
    socket.join_multicast_v4(&MULTICAST_IPV4, &Ipv4Addr::UNSPECIFIED)?;
    UdpSocket::from_std(socket.into())
}

async fn discover(socket: Arc<UdpSocket>, endpoint: Endpoint, identity: Arc<Identity>, peers: Arc<Mutex<Peers>>, tx: mpsc::Sender<Inbound>) {
    let mut buffer = vec![0; MAX_DATAGRAM_BYTES + 1];
    loop {
        let received = tokio::select! { _ = tx.closed() => break, received = socket.recv_from(&mut buffer) => received };
        let (length, from) = match received {
            | Ok(received) => received,
            | Err(_) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            },
        };
        if length > MAX_DATAGRAM_BYTES {
            continue;
        }
        let Ok(discovery) = serde_json::from_slice::<Discovery>(&buffer[.. length]) else {
            continue;
        };
        if discovery.protocol != 1 || discovery.certificate.len() > 1024 {
            continue;
        }
        let node_id = fingerprint(&discovery.certificate);
        if node_id == identity.node_id {
            continue;
        }
        let accepted = {
            let mut peers = peers.lock().unwrap();
            match &discovery.message {
                | Message::Hello {
                    node_id: declared,
                    listen_port,
                    ..
                } if *declared == node_id && *listen_port != 0 => {
                    let address = SocketAddr::new(from.ip(), *listen_port);
                    peers.discovered.retain(|_, peer| peer.seen.elapsed() <= PEER_TIMEOUT);
                    if !peers.trust.accepts(address, &node_id) {
                        continue;
                    }
                    if let Some(peer) = peers.discovered.get_mut(&address).filter(|peer| peer.node_id == node_id) {
                        peer.seen = Instant::now();
                    } else {
                        if peers.discovered.len() >= MAX_PEERS {
                            continue;
                        }
                        let previous = peers.discovered.insert(
                            address,
                            Peer {
                                node_id: node_id.clone(),
                                certificate: CertificateDer::from(discovery.certificate),
                                seen: Instant::now(),
                                link: Arc::new(AsyncMutex::new(None)),
                            },
                        );
                        match server_config(&identity, &peers) {
                            | Ok(config) => endpoint.set_server_config(Some(config)),
                            | Err(_) => {
                                peers.discovered.remove(&address);
                                if let Some(previous) = previous {
                                    peers.discovered.insert(address, previous);
                                }
                                continue;
                            },
                        }
                    }
                    true
                },
                | Message::Goodbye {
                    node_id: declared, ..
                } if *declared == node_id => {
                    let address = peers
                        .discovered
                        .iter()
                        .find(|(address, peer)| address.ip() == from.ip() && peer.node_id == node_id)
                        .map(|(address, _)| *address);
                    if let Some(address) = address {
                        peers.discovered.remove(&address);
                        true
                    } else {
                        false
                    }
                },
                | _ => false,
            }
        };
        if accepted
            && tx
                .send(Inbound {
                    from,
                    message: discovery.message,
                })
                .await
                .is_err()
        {
            break;
        }
    }
}

async fn accept_connections(endpoint: Endpoint, peers: Arc<Mutex<Peers>>, tx: mpsc::Sender<Inbound>) {
    let mut tasks = JoinSet::new();
    loop {
        tokio::select! {
            _ = tx.closed() => break,
            _ = tasks.join_next(), if !tasks.is_empty() => {},
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else { break; };
                if tasks.len() >= MAX_CONNECTIONS { incoming.refuse(); continue; }
                let peers = peers.clone();
                let tx = tx.clone();
                tasks.spawn(async move {
                    let Ok(Ok(connection)) = tokio::time::timeout(SEND_TIMEOUT, incoming).await else { return; };
                    let node_id = match authenticate(&connection, &peers) {
                        Ok(node_id) => node_id,
                        Err(_) => { connection.close(1u32.into(), b"untrusted peer"); return; }
                    };
                    if receive_chat(&connection, &node_id, tx).await.is_err() { connection.close(1u32.into(), b"invalid chat stream"); }
                });
            }
        }
    }
}

async fn receive_chat(connection: &Connection, node_id: &str, tx: mpsc::Sender<Inbound>) -> Result<(), NetworkError> {
    let (mut send, mut recv) = tokio::time::timeout(SEND_TIMEOUT, connection.accept_bi())
        .await
        .map_err(quic_error)?
        .map_err(quic_error)?;
    loop {
        let mut length = [0; 4];
        recv.read_exact(&mut length[.. 1]).await.map_err(quic_error)?;
        tokio::time::timeout(SEND_TIMEOUT, recv.read_exact(&mut length[1 ..]))
            .await
            .map_err(quic_error)?
            .map_err(quic_error)?;
        let length = u32::from_be_bytes(length) as usize;
        if length == 0 || length > MAX_DATAGRAM_BYTES {
            return Err(NetworkError::Quic("invalid chat frame length".into()));
        }
        let mut payload = vec![0; length];
        tokio::time::timeout(SEND_TIMEOUT, recv.read_exact(&mut payload))
            .await
            .map_err(quic_error)?
            .map_err(quic_error)?;
        let message = Message::decode(&payload)?;
        if !matches!(&message, Message::Chat { node_id: sender, content, .. } if sender == node_id && !content.trim().is_empty()) {
            return Err(NetworkError::Quic("chat identity does not match certificate".into()));
        }
        tx.send(Inbound {
            from: connection.remote_address(),
            message,
        })
        .await
        .map_err(quic_error)?;
        send.write_all(&[1]).await.map_err(quic_error)?;
    }
}

#[async_trait]
impl NetworkPort for QuicNetworkAdapter {
    type Error = NetworkError;

    async fn broadcast_discovery(&self, message: &Message) -> Result<(), NetworkError> {
        if !matches!(message, Message::Hello { node_id, .. } | Message::Goodbye { node_id, .. } if node_id == self.node_id()) {
            return Err(NetworkError::Quic("invalid discovery message".into()));
        }
        let payload = serde_json::to_vec(&Discovery {
            protocol: 1,
            certificate: self.identity.certificate.to_vec(),
            message: message.clone(),
        })
        .map_err(MessageError::from)?;
        if payload.len() > MAX_DATAGRAM_BYTES {
            return Err(MessageError::MessageTooLarge {
                size: payload.len(),
                max: MAX_DATAGRAM_BYTES,
            }
            .into());
        }
        self.discovery_socket
            .send_to(&payload, SocketAddrV4::new(MULTICAST_IPV4, MULTICAST_PORT))
            .await?;
        Ok(())
    }

    async fn send_direct(&self, target: SocketAddr, message: &Message) -> Result<(), NetworkError> {
        if !matches!(message, Message::Chat { node_id, .. } if node_id == self.node_id()) {
            return Err(NetworkError::Quic("invalid outgoing chat identity".into()));
        }
        self.send_chat(target, &message.encode()?).await
    }
}

#[cfg(test)]
mod tests;
