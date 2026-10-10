use async_channel::{Receiver,
                    Sender};
use p2p_app::{Inbound,
              NetworkPort,
              Node,
              NodeCommand,
              NodeEvent};
use std::time::{Duration,
                Instant,
                SystemTime};
use tokio::sync::mpsc;

const HELLO_INTERVAL: Duration = Duration::from_secs(2);
const SWEEP_INTERVAL: Duration = Duration::from_secs(3);

/// 실행 루프만 상태를 소유하고, 도메인 판단은 Node에 맡긴다.
pub(crate) async fn run_node<N: NetworkPort>(
    mut node: Node,
    network: N,
    mut inbound: mpsc::Receiver<Inbound>,
    commands: Receiver<NodeCommand>,
    events: Sender<NodeEvent>,
) {
    let mut hello_tick = tokio::time::interval(HELLO_INTERVAL);
    let mut sweep_tick = tokio::time::interval(SWEEP_INTERVAL);
    loop {
        tokio::select! {
            _ = hello_tick.tick() => {
                let _ = network.broadcast_discovery(&node.hello()).await;
            },
            _ = sweep_tick.tick() => {
                for event in node.expire(Instant::now()) {
                    emit(&events, event);
                }
            },
            received = inbound.recv() => match received {
                Some(inbound) => {
                    if let Some(event) = node.receive(inbound, Instant::now(), SystemTime::now()) {
                        emit(&events, event);
                    }
                },
                None => break,
            },
            command = commands.recv() => match command {
                Ok(NodeCommand::SendChat(text)) => send_chat(&node, &network, &text, &events).await,
                Ok(NodeCommand::Shutdown) | Err(_) => break,
            },
        }
    }
    let _ = network.broadcast_discovery(&node.goodbye()).await;
    emit(&events, NodeEvent::Stopped);
}

async fn send_chat<N: NetworkPort>(node: &Node, network: &N, text: &str, events: &Sender<NodeEvent>) {
    let dispatch = match node.prepare_chat(text) {
        | Ok(dispatch) => dispatch,
        | Err(error) => {
            emit(events, NodeEvent::SendFailed(error));
            return;
        },
    };
    let mut sent_to = 0;
    let mut last_error = String::new();
    for &target in dispatch.targets() {
        match network.send_direct(target, dispatch.message()).await {
            | Ok(()) => sent_to += 1,
            | Err(error) => last_error = error.to_string(),
        }
    }
    emit(events, dispatch.finish(sent_to, last_error, SystemTime::now()));
}

/// 이벤트 채널은 무제한이므로 실패는 UI가 이미 닫혔다는 뜻이다.
fn emit(events: &Sender<NodeEvent>, event: NodeEvent) { let _ = events.try_send(event); }

#[cfg(test)]
mod tests;
