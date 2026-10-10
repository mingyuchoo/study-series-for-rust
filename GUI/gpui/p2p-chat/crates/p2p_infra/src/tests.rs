use super::*;
use crate::quic::{SERVER_NAME,
                  client_config};
use p2p_core::MAX_DATAGRAM_BYTES;
use std::time::Duration;
use tempfile::TempDir;

struct TestNode {
    network: QuicNetworkAdapter,
    inbound: mpsc::Receiver<Inbound>,
    state: TempDir,
}

impl TestNode {
    async fn new() -> Self {
        let state = tempfile::tempdir().unwrap();
        let (network, inbound) = QuicNetworkAdapter::bind(0, state.path()).await.unwrap();
        Self {
            network,
            inbound,
            state,
        }
    }

    fn hello(&self) -> Message { Message::hello(self.network.node_id(), "test", self.network.listen_port()) }

    fn chat(&self, text: &str) -> Message { Message::chat(self.network.node_id(), "test", text) }
}

async fn discovered(node: &mut TestNode, id: &str) -> SocketAddr {
    loop {
        let inbound = node.inbound.recv().await.unwrap();
        if let Message::Hello {
            node_id,
            listen_port,
            ..
        } = inbound.message
        {
            if node_id == id {
                return SocketAddr::new(inbound.from.ip(), listen_port);
            }
        }
    }
}

async fn pair() -> (TestNode, TestNode, SocketAddr, SocketAddr) {
    let mut alice = TestNode::new().await;
    let mut bob = TestNode::new().await;
    alice.network.broadcast_discovery(&alice.hello()).await.unwrap();
    bob.network.broadcast_discovery(&bob.hello()).await.unwrap();
    let alice_id = alice.network.node_id().to_string();
    let bob_id = bob.network.node_id().to_string();
    let (bob_addr, alice_addr) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(discovered(&mut alice, &bob_id), discovered(&mut bob, &alice_id))
    })
    .await
    .unwrap();
    (alice, bob, alice_addr, bob_addr)
}

async fn next_chat(node: &mut TestNode) -> Message {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let inbound = node.inbound.recv().await.unwrap();
            if matches!(inbound.message, Message::Chat { .. }) {
                return inbound.message;
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn multicast_discovery_and_mutual_tls_chat_preserve_order_and_reuse_connections() {
    let (mut alice, mut bob, alice_addr, bob_addr) = pair().await;
    for i in 0 .. 10 {
        let message = alice.chat(&format!("안녕 {i}"));
        alice.network.send_direct(bob_addr, &message).await.unwrap();
        assert_eq!(next_chat(&mut bob).await, message);
    }
    assert_eq!(alice.network.endpoint.open_connections(), 1);
    let reply = bob.chat("반가워요");
    bob.network.send_direct(alice_addr, &reply).await.unwrap();
    assert_eq!(next_chat(&mut alice).await, reply);
    let pins: HashMap<String, String> =
        serde_json::from_slice(&std::fs::read(alice.state.path().join(alice.network.listen_port().to_string()).join("trusted-peers.json")).unwrap()).unwrap();
    assert_eq!(pins[&bob_addr.to_string()], bob.network.node_id());
}

#[tokio::test]
async fn tls_rejects_a_server_certificate_other_than_the_discovered_certificate() {
    let (alice, _bob, _, bob_addr) = pair().await;
    let stranger = TestNode::new().await;
    let client = client_config(&alice.network.identity, stranger.network.identity.certificate.clone()).unwrap();
    let connecting = alice.network.endpoint.connect_with(client, bob_addr, SERVER_NAME).unwrap();
    let result = tokio::time::timeout(SEND_TIMEOUT, connecting).await.unwrap();
    assert!(result.is_err(), "a certificate signed by the wrong peer must fail TLS verification");
}

#[tokio::test]
async fn tls_rejects_an_undiscovered_client_certificate() {
    let (_alice, bob, _, bob_addr) = pair().await;
    let stranger = TestNode::new().await;
    let client = client_config(&stranger.network.identity, bob.network.identity.certificate.clone()).unwrap();
    let connection = stranger.network.endpoint.connect_with(client, bob_addr, SERVER_NAME).unwrap().await.unwrap();
    let closed = tokio::time::timeout(SEND_TIMEOUT, connection.closed()).await.unwrap();
    assert!(matches!(
        closed,
        quinn::ConnectionError::TransportError(_) | quinn::ConnectionError::ConnectionClosed(_)
    ));
}

#[tokio::test]
async fn receiver_rejects_spoofed_identity_and_oversized_frames() {
    let (alice, mut bob, _, bob_addr) = pair().await;
    for payload in [Some(Message::chat("forged", "test", "forged").encode().unwrap()), None] {
        let client = client_config(&alice.network.identity, bob.network.identity.certificate.clone()).unwrap();
        let connection = alice.network.endpoint.connect_with(client, bob_addr, SERVER_NAME).unwrap().await.unwrap();
        let (mut send, mut recv) = connection.open_bi().await.unwrap();
        let length = payload.as_ref().map_or(MAX_DATAGRAM_BYTES + 1, Vec::len) as u32;
        send.write_all(&length.to_be_bytes()).await.unwrap();
        if let Some(payload) = payload {
            send.write_all(&payload).await.unwrap();
        }
        assert!(tokio::time::timeout(SEND_TIMEOUT, recv.read_exact(&mut [0])).await.unwrap().is_err());
        assert!(!std::iter::from_fn(|| bob.inbound.try_recv().ok()).any(|inbound| matches!(inbound.message, Message::Chat { .. })));
    }
}

#[tokio::test]
async fn send_times_out_when_a_discovered_peer_never_answers() {
    let (alice, bob, _, bob_addr) = pair().await;
    let port = bob.network.listen_port();
    drop(bob);
    let _blackhole = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            match std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)) {
                | Ok(socket) => break socket,
                | Err(error) if error.kind() == io::ErrorKind::AddrInUse => tokio::time::sleep(Duration::from_millis(10)).await,
                | Err(error) => panic!("{error}"),
            }
        }
    })
    .await
    .unwrap();
    let result = tokio::time::timeout(
        SEND_TIMEOUT + Duration::from_secs(1),
        alice.network.send_direct(bob_addr, &alice.chat("timeout")),
    )
    .await
    .unwrap();
    assert!(matches!(result, Err(NetworkError::Quic(error)) if error.contains("timed out")));
}

#[tokio::test]
async fn legacy_udp_chat_cannot_enter_the_chat_channel() {
    let mut node = TestNode::new().await;
    let sender = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap();
    sender
        .send_to(
            &Message::chat("forged", "test", "plaintext").encode().unwrap(),
            SocketAddrV4::new(MULTICAST_IPV4, MULTICAST_PORT),
        )
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), async {
            loop {
                if matches!(node.inbound.recv().await.unwrap().message, Message::Chat { .. }) {
                    break;
                }
            }
        })
        .await
        .is_err()
    );
}

#[tokio::test]
async fn cancelling_an_unacknowledged_send_closes_the_stream_and_the_next_send_reconnects() {
    let (alice, mut bob, _, bob_addr) = pair().await;
    bob.network.tasks[1].abort();
    let _ = (&mut bob.network.tasks[1]).await;
    let endpoint = bob.network.endpoint.clone();
    let expected_id = alice.network.node_id().to_string();
    let (received_tx, mut received_rx) = mpsc::channel(1);
    let server = tokio::spawn(async move {
        for attempt in 0 .. 2 {
            let connection = endpoint.accept().await.unwrap().await.unwrap();
            let (mut send, mut recv) = connection.accept_bi().await.unwrap();
            let mut length = [0; 4];
            recv.read_exact(&mut length).await.unwrap();
            let mut bytes = vec![0; u32::from_be_bytes(length) as usize];
            recv.read_exact(&mut bytes).await.unwrap();
            if attempt == 0 {
                received_tx.send(()).await.unwrap();
                connection.closed().await;
            } else {
                assert_eq!(Message::decode(&bytes).unwrap(), Message::chat(&expected_id, "test", "second"));
                send.write_all(&[1]).await.unwrap();
                connection.closed().await;
            }
        }
    });
    {
        let message = alice.chat("cancelled");
        let send = alice.network.send_direct(bob_addr, &message);
        tokio::pin!(send);
        tokio::select! {
            result = &mut send => panic!("the server withheld its ACK: {result:?}"),
            received = received_rx.recv() => assert_eq!(received, Some(())),
        }
    }
    let link = alice.network.peers.lock().unwrap().discovered[&bob_addr].link.clone();
    assert!(
        link.lock().await.is_none(),
        "a cancelled send must not leave a partially used stream in the cache"
    );
    alice.network.send_direct(bob_addr, &alice.chat("second")).await.unwrap();
    drop(alice);
    tokio::time::timeout(SEND_TIMEOUT, server).await.unwrap().unwrap();
}

#[test]
fn identity_and_pins_survive_restart_and_changed_certificates_are_rejected() {
    let state = tempfile::tempdir().unwrap();
    let first = Identity::load(state.path()).unwrap();
    assert_eq!(first.node_id, Identity::load(state.path()).unwrap().node_id);
    let peer: SocketAddr = "127.0.0.1:9001".parse().unwrap();
    TrustStore::load(state.path()).unwrap().pin(peer, &first.node_id).unwrap();
    let mut trust = TrustStore::load(state.path()).unwrap();
    assert!(trust.accepts(peer, &first.node_id));
    assert!(!trust.accepts(peer, "changed"));
    assert!(trust.pin(peer, "changed").is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(state.path().join("identity.json")).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn corrupt_identity_and_trust_files_fail_closed() {
    let state = tempfile::tempdir().unwrap();
    std::fs::write(state.path().join("identity.json"), "corrupt").unwrap();
    assert!(Identity::load(state.path()).is_err());
    std::fs::write(state.path().join("trusted-peers.json"), "corrupt").unwrap();
    assert!(TrustStore::load(state.path()).is_err());
}
