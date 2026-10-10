use async_trait::async_trait;
use p2p_core::{MAX_DATAGRAM_BYTES,
               Message,
               NetworkPort,
               P2PError};
use socket2::{Domain,
              Protocol,
              Socket,
              Type};
use std::{net::{Ipv4Addr,
                SocketAddr,
                SocketAddrV4},
          sync::Arc,
          time::Duration};
use tokio::{net::UdpSocket,
            sync::mpsc};

pub const MULTICAST_IPV4: Ipv4Addr = Ipv4Addr::new(239, 255, 42, 99);
pub const MULTICAST_PORT: u16 = 50050;

/// 수신 큐 용량. 가득 차면 수신 루프가 소비될 때까지 기다린다.
const INBOUND_QUEUE_CAPACITY: usize = 256;
/// 수신 오류가 연속으로 날 때 CPU를 점유하지 않도록 잠시 쉰다.
const RECV_ERROR_BACKOFF: Duration = Duration::from_millis(50);

/// 디코딩이 끝난 수신 메시지와 보낸 곳의 주소
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inbound {
    pub from: SocketAddr,
    pub message: Message,
}

/// UDP 기반의 네트워크 인프라 구현체 (부수효과/I/O 격리)
pub struct UdpNetworkAdapter {
    direct_socket: Arc<UdpSocket>,
    multicast_dest: SocketAddr,
}

impl UdpNetworkAdapter {
    /// 직접 통신 소켓과 멀티캐스트 디스커버리 소켓을 바인딩하고 수신 루프를
    /// 시작한다. 수신 메시지는 반환되는 채널로 전달되며, 소켓은 어댑터
    /// 밖으로 노출하지 않는다.
    pub async fn bind(listen_port: u16) -> Result<(Self, mpsc::Receiver<Inbound>), P2PError> {
        // 1. 직접 통신용 UDP 소켓 (CHAT 수신, HELLO/GOODBYE 멀티캐스트 송신)
        let direct_socket = Arc::new(UdpSocket::bind(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), listen_port)).await?);
        // 2. 멀티캐스트 디스커버리 수신용 소켓
        let discovery_socket = Arc::new(bind_multicast()?);

        let (tx, rx) = mpsc::channel(INBOUND_QUEUE_CAPACITY);
        tokio::spawn(receive_loop(discovery_socket, tx.clone()));
        tokio::spawn(receive_loop(direct_socket.clone(), tx));

        let adapter = Self {
            direct_socket,
            multicast_dest: SocketAddr::V4(SocketAddrV4::new(MULTICAST_IPV4, MULTICAST_PORT)),
        };
        Ok((adapter, rx))
    }
}

/// 멀티캐스트 그룹에 가입한 수신용 소켓. SO_REUSEADDR로 같은 PC의 여러 노드가
/// 같은 포트를 공유한다.
fn bind_multicast() -> Result<UdpSocket, P2PError> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(not(windows))]
    socket.set_reuse_port(true)?;
    socket.set_nonblocking(true)?;

    socket.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, MULTICAST_PORT).into())?;
    socket.join_multicast_v4(&MULTICAST_IPV4, &Ipv4Addr::UNSPECIFIED)?;

    let std_socket: std::net::UdpSocket = socket.into();
    Ok(UdpSocket::from_std(std_socket)?)
}

/// 소켓에서 데이터그램을 읽어 디코딩한 뒤 채널로 보낸다.
/// 잘못된 데이터그램과 일시적인 수신 오류(예: Windows의 WSAECONNRESET)는 버리고
/// 계속 읽는다. 수신자가 사라지면 루프를 끝낸다.
async fn receive_loop(socket: Arc<UdpSocket>, tx: mpsc::Sender<Inbound>) {
    let mut buf = vec![0u8; MAX_DATAGRAM_BYTES];
    loop {
        match socket.recv_from(&mut buf).await {
            | Ok((len, from)) => {
                let Ok(message) = Message::decode(&buf[.. len]) else {
                    continue;
                };
                if tx
                    .send(Inbound {
                        from,
                        message,
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            },
            | Err(_) => tokio::time::sleep(RECV_ERROR_BACKOFF).await,
        }
    }
}

#[async_trait]
impl NetworkPort for UdpNetworkAdapter {
    async fn broadcast_discovery(&self, message: &Message) -> Result<(), P2PError> {
        let payload = message.encode()?;
        self.direct_socket.send_to(&payload, self.multicast_dest).await?;
        Ok(())
    }

    async fn send_direct(&self, target: SocketAddr, message: &Message) -> Result<(), P2PError> {
        let payload = message.encode()?;
        self.direct_socket.send_to(&payload, target).await?;
        Ok(())
    }
}
