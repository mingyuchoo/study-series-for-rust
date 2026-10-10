//! 상호 TLS 인증, 연결 재사용, QUIC 채팅 프레임과 ACK의 I/O를 담당한다.

use crate::{NetworkError,
            identity::{Identity,
                       TrustStore},
            protocol::{decode_chat,
                       fingerprint,
                       validate_frame_length}};
use p2p_app::Inbound;
use quinn::{Connection,
            Endpoint,
            RecvStream,
            SendStream,
            crypto::rustls::{QuicClientConfig,
                             QuicServerConfig}};
use rustls::{RootCertStore,
             pki_types::CertificateDer};
use std::{collections::HashMap,
          io,
          net::SocketAddr,
          sync::{Arc,
                 Mutex},
          time::{Duration,
                 Instant}};
use tokio::{sync::{Mutex as AsyncMutex,
                   mpsc},
            task::JoinSet};

pub const SEND_TIMEOUT: Duration = Duration::from_secs(3);
pub(super) const SERVER_NAME: &str = "p2p-chat";
const ALPN: &[u8] = b"p2p-chat/1";
const MAX_CONNECTIONS: usize = 64;
pub(super) const PEER_TIMEOUT: Duration = Duration::from_secs(6);

pub(super) struct Link {
    connection: Connection,
    send: SendStream,
    recv: RecvStream,
}

impl Drop for Link {
    fn drop(&mut self) { self.connection.close(0u32.into(), b"chat stream ended"); }
}

pub(super) struct Peer {
    pub(super) node_id: String,
    pub(super) certificate: CertificateDer<'static>,
    pub(super) seen: Instant,
    pub(super) link: Arc<AsyncMutex<Option<Link>>>,
}

pub(super) struct Peers {
    pub(super) discovered: HashMap<SocketAddr, Peer>,
    pub(super) trust: TrustStore,
}

pub(super) async fn send_chat(endpoint: &Endpoint, identity: &Identity, peers: &Mutex<Peers>, target: SocketAddr, payload: &[u8]) -> Result<(), NetworkError> {
    let (certificate, link) = {
        let peers = peers.lock().unwrap();
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
                let client = client_config(identity, certificate)?;
                let connection = endpoint
                    .connect_with(client, target, SERVER_NAME)
                    .map_err(quic_error)?
                    .await
                    .map_err(quic_error)?;
                authenticate(&connection, peers)?;
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

fn quic_error(error: impl std::fmt::Display) -> NetworkError { NetworkError::Quic(error.to_string()) }

pub(super) fn server_config(identity: &Identity, peers: &Peers) -> io::Result<quinn::ServerConfig> {
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

pub(super) fn client_config(identity: &Identity, certificate: CertificateDer<'static>) -> io::Result<quinn::ClientConfig> {
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

pub(super) async fn accept_connections(endpoint: Endpoint, peers: Arc<Mutex<Peers>>, tx: mpsc::Sender<Inbound>) {
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
        let length = validate_frame_length(u32::from_be_bytes(length) as usize)?;
        let mut payload = vec![0; length];
        tokio::time::timeout(SEND_TIMEOUT, recv.read_exact(&mut payload))
            .await
            .map_err(quic_error)?
            .map_err(quic_error)?;
        let message = decode_chat(&payload, node_id)?;
        tx.send(Inbound {
            from: connection.remote_address(),
            message,
        })
        .await
        .map_err(quic_error)?;
        send.write_all(&[1]).await.map_err(quic_error)?;
    }
}
