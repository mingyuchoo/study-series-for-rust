use async_channel::{Receiver,
                    Sender};
use p2p_app::{ChatDispatch,
              Inbound,
              NetworkPort,
              Node,
              NodeCommand,
              NodeEvent};
use p2p_infra::SEND_TIMEOUT;
use std::{sync::Arc,
          time::{Duration,
                 Instant,
                 SystemTime}};
use tokio::{sync::mpsc,
            task::JoinSet};

const HELLO_INTERVAL: Duration = Duration::from_secs(2);
const SWEEP_INTERVAL: Duration = Duration::from_secs(3);
const SEND_QUEUE_CAPACITY: usize = 32;

/// 실행 루프만 상태를 소유하고, 네트워크 전송 대기는 별도 작업에 맡긴다.
pub(crate) async fn run_node<N: NetworkPort + 'static>(
    mut node: Node,
    network: N,
    mut inbound: mpsc::Receiver<Inbound>,
    commands: Receiver<NodeCommand>,
    events: Sender<NodeEvent>,
) {
    let network = Arc::new(network);
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<ChatDispatch>(SEND_QUEUE_CAPACITY);
    let (completed_tx, mut completed_rx) = mpsc::channel(SEND_QUEUE_CAPACITY);
    let sender_network = network.clone();
    // shortcut: 전송 큐 하나로 메시지 순서를 유지한다. 지속적인 트래픽이
    // 필요하면 peer별 큐로 나눈다.
    let mut sender = tokio::spawn(async move {
        while let Some(dispatch) = outbound_rx.recv().await {
            let event = send_chat(dispatch, sender_network.clone()).await;
            if completed_tx.send(event).await.is_err() {
                break;
            }
        }
    });
    let mut pending = 0;
    let mut hello_tick = tokio::time::interval(HELLO_INTERVAL);
    let mut sweep_tick = tokio::time::interval(SWEEP_INTERVAL);
    loop {
        tokio::select! {
            _ = hello_tick.tick() => {
                let _ = network.broadcast_discovery(&node.hello()).await;
            },
            _ = sweep_tick.tick() => {
                for event in node.expire(Instant::now()) { emit(&events, event); }
            },
            received = inbound.recv() => match received {
                Some(inbound) => {
                    if let Some(event) = node.receive(inbound, Instant::now(), SystemTime::now()) { emit(&events, event); }
                },
                None => break,
            },
            completed = completed_rx.recv(), if pending > 0 => match completed {
                Some(event) => { pending -= 1; emit(&events, event); },
                None => { emit(&events, NodeEvent::SendFailed("전송 작업이 종료되었습니다.".into())); break; }
            },
            command = commands.recv() => match command {
                Ok(NodeCommand::SendChat(text)) => match node.prepare_chat(&text) {
                    Ok(dispatch) => match outbound_tx.try_send(dispatch) {
                        Ok(()) => pending += 1,
                        Err(_) => emit(&events, NodeEvent::SendFailed("전송 대기열이 가득 찼습니다. 잠시 후 다시 보내세요.".into())),
                    },
                    Err(error) => emit(&events, NodeEvent::SendFailed(error)),
                },
                Ok(NodeCommand::Shutdown) | Err(_) => break,
            },
        }
    }
    drop(outbound_tx);
    let deadline = tokio::time::Instant::now() + SEND_TIMEOUT;
    while pending > 0 {
        match tokio::time::timeout_at(deadline, completed_rx.recv()).await {
            | Ok(Some(event)) => {
                pending -= 1;
                emit(&events, event);
            },
            | _ => break,
        }
    }
    sender.abort();
    let _ = (&mut sender).await;
    while let Ok(event) = completed_rx.try_recv() {
        pending -= 1;
        emit(&events, event);
    }
    if pending > 0 {
        emit(
            &events,
            NodeEvent::SendFailed(format!("노드 종료로 메시지 {pending}개의 전송이 취소되었습니다.")),
        );
    }
    let _ = tokio::time::timeout(SEND_TIMEOUT, network.broadcast_discovery(&node.goodbye())).await;
    emit(&events, NodeEvent::Stopped);
}

async fn send_chat<N: NetworkPort + 'static>(dispatch: ChatDispatch, network: Arc<N>) -> NodeEvent {
    let mut sends = JoinSet::new();
    for &target in dispatch.targets() {
        let network = network.clone();
        let message = dispatch.message().clone();
        sends.spawn(async move {
            match tokio::time::timeout(SEND_TIMEOUT, network.send_direct(target, &message)).await {
                | Ok(result) => result.map_err(|error| error.to_string()),
                | Err(_) => Err(format!("{target} 전송 시간이 초과되었습니다.")),
            }
        });
    }
    let mut sent_to = 0;
    let mut last_error = String::new();
    while let Some(result) = sends.join_next().await {
        match result {
            | Ok(Ok(())) => sent_to += 1,
            | Ok(Err(error)) => last_error = error,
            | Err(error) => last_error = error.to_string(),
        }
    }
    dispatch.finish(sent_to, last_error, SystemTime::now())
}

/// 이벤트 채널은 무제한이므로 실패는 UI가 이미 닫혔다는 뜻이다.
fn emit(events: &Sender<NodeEvent>, event: NodeEvent) { let _ = events.try_send(event); }

#[cfg(test)]
mod tests;
