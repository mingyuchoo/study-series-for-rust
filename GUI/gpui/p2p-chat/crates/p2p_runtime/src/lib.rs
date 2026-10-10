//! 부수효과 실행과 의존성 조립: 스레드, Tokio, QUIC, 채널.

mod runner;

use async_channel::{Receiver,
                    Sender};
use p2p_app::{ConfigError,
              Node,
              NodeCommand,
              NodeConfig,
              NodeEvent};
use p2p_infra::QuicNetworkAdapter;
use std::{path::Path,
          thread::JoinHandle};
use thiserror::Error;

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
    let directory = dirs::data_local_dir().ok_or_else(|| StartError::Startup("사용자 데이터 디렉터리를 찾을 수 없습니다.".into()))?;
    start_with_state_dir(config, directory.join("p2p-chat"))
}

/// 별도 프로필 또는 격리된 통합 테스트를 위해 상태 저장 위치를 지정한다.
pub fn start_with_state_dir(config: NodeConfig, state_directory: impl AsRef<Path>) -> Result<NodeSession, StartError> {
    let config = NodeConfig::new(config.nickname, config.listen_port)?;
    let listen_port = config.listen_port;
    let directory = state_directory.as_ref().to_path_buf();
    let (command_tx, command_rx) = async_channel::unbounded();
    let (event_tx, event_rx) = async_channel::unbounded();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<String, StartError>>();

    let thread = std::thread::Builder::new().name("p2p-node".into()).spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            | Ok(runtime) => runtime,
            | Err(e) => {
                let _ = ready_tx.send(Err(StartError::Io(e)));
                return;
            },
        };
        runtime.block_on(async move {
            match QuicNetworkAdapter::bind(listen_port, &directory).await {
                | Ok((adapter, inbound)) => {
                    let node_id = adapter.node_id().to_string();
                    let node = match Node::new(config, node_id.clone()) {
                        | Ok(node) => node,
                        | Err(error) => {
                            let _ = ready_tx.send(Err(error.into()));
                            return;
                        },
                    };
                    let _ = ready_tx.send(Ok(node_id));
                    runner::run_node(node, adapter, inbound, command_rx, event_tx).await;
                },
                | Err(e) => {
                    let _ = ready_tx.send(Err(e.into()));
                },
            }
        });
    })?;

    match ready_rx.recv() {
        | Ok(Ok(node_id)) => Ok(NodeSession {
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
