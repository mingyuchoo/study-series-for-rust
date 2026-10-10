//! UDP 멀티캐스트 수신과 발견한 피어의 QUIC 신뢰 설정 갱신을 담당한다.

use crate::{identity::Identity,
            protocol::{Discovery,
                       fingerprint},
            quic::{PEER_TIMEOUT,
                   Peer,
                   Peers,
                   server_config}};
use p2p_app::Inbound;
use p2p_core::{MAX_DATAGRAM_BYTES,
               Message};
use quinn::Endpoint;
use rustls::pki_types::CertificateDer;
use socket2::{Domain,
              Protocol,
              Socket,
              Type};
use std::{io,
          net::{Ipv4Addr,
                SocketAddr,
                SocketAddrV4},
          sync::{Arc,
                 Mutex},
          time::{Duration,
                 Instant}};
use tokio::{net::UdpSocket,
            sync::{Mutex as AsyncMutex,
                   mpsc}};

pub const MULTICAST_IPV4: Ipv4Addr = Ipv4Addr::new(239, 255, 42, 99);
pub const MULTICAST_PORT: u16 = 50050;
const MAX_PEERS: usize = 256;

pub(super) fn bind_multicast() -> io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(not(windows))]
    socket.set_reuse_port(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, MULTICAST_PORT).into())?;
    socket.join_multicast_v4(&MULTICAST_IPV4, &Ipv4Addr::UNSPECIFIED)?;
    UdpSocket::from_std(socket.into())
}

pub(super) async fn discover(socket: Arc<UdpSocket>, endpoint: Endpoint, identity: Arc<Identity>, peers: Arc<Mutex<Peers>>, tx: mpsc::Sender<Inbound>) {
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
        let Some(discovery) = Discovery::decode(&buffer[.. length]) else {
            continue;
        };
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
