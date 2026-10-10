//! 부수효과 실행과 의존성 조립: UUID, 스레드, Tokio, UDP, 채널.

mod runner;

use async_channel::{Receiver,
                    Sender};
use p2p_app::{ConfigError,
              Node,
              NodeCommand,
              NodeConfig,
              NodeEvent};
use p2p_infra::UdpNetworkAdapter;
use std::thread::JoinHandle;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum StartError {
    #[error("{0}")]
    Config(#[from] ConfigError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Startup(String),
}

/// 실행 중인 노드. `commands`로 명령을 보내고 `events`에서 이벤트를 받는다.
pub struct NodeSession {
    pub node_id: String,
    pub commands: Sender<NodeCommand>,
    pub events: Receiver<NodeEvent>,
    thread: JoinHandle<()>,
}

impl NodeSession {
    /// 종료 명령을 보내고, 노드가 GOODBYE를 보내고 끝날 때까지 기다린다.
    pub fn shutdown(self) {
        let _ = self.commands.try_send(NodeCommand::Shutdown);
        let _ = self.thread.join();
    }
}

/// 노드를 전용 스레드(자체 tokio 런타임)에서 시작한다.
/// 소켓 바인딩에 성공하면 세션을 돌려주고, 실패하면 그 오류를 돌려준다.
pub fn start(config: NodeConfig) -> Result<NodeSession, StartError> {
    let node_id = Uuid::new_v4().to_string();
    let listen_port = config.listen_port;
    let node = Node::new(config, node_id.clone())?;
    let (command_tx, command_rx) = async_channel::unbounded();
    let (event_tx, event_rx) = async_channel::unbounded();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), StartError>>();

    let thread = std::thread::Builder::new().name("p2p-node".into()).spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            | Ok(runtime) => runtime,
            | Err(e) => {
                let _ = ready_tx.send(Err(StartError::Io(e)));
                return;
            },
        };
        runtime.block_on(async move {
            match UdpNetworkAdapter::bind(listen_port).await {
                | Ok((adapter, inbound)) => {
                    let _ = ready_tx.send(Ok(()));
                    runner::run_node(node, adapter, inbound, command_rx, event_tx).await;
                },
                | Err(e) => {
                    let _ = ready_tx.send(Err(e.into()));
                },
            }
        });
    })?;

    match ready_rx.recv() {
        | Ok(Ok(())) => Ok(NodeSession {
            node_id,
            commands: command_tx,
            events: event_rx,
            thread,
        }),
        | Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        },
        | Err(_) => {
            let _ = thread.join();
            Err(StartError::Startup("노드 스레드가 시작되지 않았습니다.".into()))
        },
    }
}
