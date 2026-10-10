//! 앱 상태와 동작: 설정 입력 → 노드 시작 → 채팅.

use crate::text_input::TextInput;
use async_channel::{Receiver,
                    Sender};
use gpui::{AppContext,
           Context,
           Entity,
           Focusable,
           ScrollHandle,
           Window,
           actions};
use p2p_app::{LeaveReason,
              NodeCommand,
              NodeConfig,
              NodeEvent,
              NodeSession,
              PeerInfo};
use p2p_core::{ChatEntry,
               P2PError,
               Speaker};
use std::{cell::RefCell,
          rc::Rc,
          time::SystemTime};

actions!(p2p_chat, [Submit]);

/// 닉네임 최대 길이(글자 수). HELLO 메시지가 데이터그램 한도를 넘지 않도록
/// 제한한다.
pub const MAX_NICKNAME_CHARS: usize = 20;
pub const DEFAULT_PORT: u16 = 9001;

/// 노드가 실행 중일 때의 채팅 화면 상태
pub struct ChatState {
    pub node_id: String,
    pub nickname: String,
    pub listen_port: u16,
    pub commands: Sender<NodeCommand>,
    pub peers: Vec<PeerInfo>,
    pub entries: Vec<ChatEntry>,
    pub error: Option<String>,
    pub stopped: bool,
    pub message_input: Entity<TextInput>,
    pub scroll: ScrollHandle,
}

pub struct AppRoot {
    pub(crate) nickname_input: Entity<TextInput>,
    pub(crate) port_input: Entity<TextInput>,
    pub(crate) setup_error: Option<String>,
    pub(crate) chat: Option<ChatState>,
    /// 노드 세션의 소유권을 main과 공유한다. 창을 닫은 뒤 main에서 `shutdown`을
    /// 호출하기 위함이다.
    session_slot: Rc<RefCell<Option<NodeSession>>>,
}

impl AppRoot {
    pub fn new(session_slot: Rc<RefCell<Option<NodeSession>>>, cx: &mut Context<Self>) -> Self {
        Self {
            nickname_input: cx.new(|cx| TextInput::new("닉네임", "", cx)),
            port_input: cx.new(|cx| TextInput::new("포트", &DEFAULT_PORT.to_string(), cx)),
            setup_error: None,
            chat: None,
            session_slot,
        }
    }

    /// 창을 열었을 때 첫 입력창에 포커스를 준다.
    pub fn focus_first_field(&self, window: &mut Window, cx: &mut Context<Self>) { window.focus(&self.nickname_input.focus_handle(cx)); }

    /// Enter 키: 설정 화면이면 노드를 시작하고, 채팅 화면이면 메시지를 보낸다.
    pub(crate) fn submit(&mut self, _: &Submit, window: &mut Window, cx: &mut Context<Self>) {
        if self.chat.is_some() {
            self.send_chat(cx);
        } else {
            self.start_session(window, cx);
        }
    }

    /// 입력값을 검사한 뒤 노드를 시작하고 채팅 화면으로 전환한다.
    pub(crate) fn start_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let nickname = self.nickname_input.read(cx).text().trim().to_string();
        let port_text = self.port_input.read(cx).text();

        if nickname.is_empty() {
            return self.fail_setup("닉네임을 입력하세요.", cx);
        }
        if nickname.chars().count() > MAX_NICKNAME_CHARS {
            return self.fail_setup(format!("닉네임은 {MAX_NICKNAME_CHARS}자 이하로 입력하세요."), cx);
        }
        let listen_port = match port_text.trim().parse::<u16>() {
            | Ok(port) if port > 0 => port,
            | _ => return self.fail_setup("포트는 1~65535 사이의 숫자여야 합니다.", cx),
        };

        match p2p_app::start(NodeConfig {
            nickname: nickname.clone(),
            listen_port,
        }) {
            | Ok(session) => {
                let commands = session.commands.clone();
                let events = session.events.clone();
                let node_id = session.node_id.clone();
                *self.session_slot.borrow_mut() = Some(session);

                let message_input = cx.new(|cx| TextInput::new("메시지를 입력하세요", "", cx));
                window.focus(&message_input.focus_handle(cx));

                self.chat = Some(ChatState {
                    node_id,
                    nickname,
                    listen_port,
                    commands,
                    peers: Vec::new(),
                    entries: Vec::new(),
                    error: None,
                    stopped: false,
                    message_input,
                    scroll: ScrollHandle::new(),
                });
                self.setup_error = None;
                self.spawn_event_loop(events, cx);
                cx.notify();
            },
            | Err(P2PError::Io(e)) if e.kind() == std::io::ErrorKind::AddrInUse =>
                self.fail_setup(format!("포트 {listen_port}은(는) 이미 사용 중입니다. 다른 포트를 입력하세요."), cx),
            | Err(e) => self.fail_setup(format!("노드를 시작하지 못했습니다: {e}"), cx),
        }
    }

    /// 입력창의 메시지를 모든 피어에게 보낸다. 실제 전송 결과는 이벤트로
    /// 돌아온다.
    pub(crate) fn send_chat(&mut self, cx: &mut Context<Self>) {
        let Some(chat) = self.chat.as_mut() else {
            return;
        };
        let text = chat.message_input.read(cx).text();
        if text.trim().is_empty() {
            return;
        }
        if chat.stopped {
            chat.error = Some("노드가 종료되어 메시지를 보낼 수 없습니다.".into());
        } else if chat.peers.is_empty() {
            chat.error = Some("연결된 피어가 없습니다. 상대가 같은 네트워크에서 실행 중인지 확인하세요.".into());
        } else {
            chat.error = None;
            let _ = chat.commands.try_send(NodeCommand::SendChat(text));
            let input = chat.message_input.clone();
            input.update(cx, |input, cx| input.clear(cx));
        }
        cx.notify();
    }

    fn fail_setup(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.setup_error = Some(message.into());
        cx.notify();
    }

    /// 노드 이벤트를 UI 스레드에서 받아 상태에 반영한다. 노드가 끝나면 루프도
    /// 끝난다.
    fn spawn_event_loop(&self, events: Receiver<NodeEvent>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                if this.update(cx, |root, cx| root.on_node_event(event, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn on_node_event(&mut self, event: NodeEvent, cx: &mut Context<Self>) {
        let Some(chat) = self.chat.as_mut() else {
            return;
        };
        // 새 대화 항목이 생겼을 때만 맨 아래로 스크롤한다. 피어 갱신(HELLO)마다
        // 스크롤을 내리면 읽던 위치가 튄다.
        let mut appended = true;
        match event {
            | NodeEvent::PeerJoined(peer) => {
                chat.entries.push(notice(format!("'{}' 입장", peer.nickname)));
                chat.peers.push(peer);
            },
            | NodeEvent::PeerUpdated(peer) => {
                appended = false;
                match chat.peers.iter_mut().find(|p| p.id == peer.id) {
                    | Some(existing) => *existing = peer,
                    | None => chat.peers.push(peer),
                }
            },
            | NodeEvent::PeerLeft {
                id,
                nickname,
                reason,
            } => {
                chat.peers.retain(|p| p.id != id);
                let text = match reason {
                    | LeaveReason::Goodbye => format!("'{nickname}' 퇴장"),
                    | LeaveReason::Timeout => format!("'{nickname}' 응답 없음 (연결 끊김)"),
                };
                chat.entries.push(notice(text));
            },
            | NodeEvent::ChatReceived {
                id,
                nickname,
                content,
                at,
            } => {
                chat.entries.push(ChatEntry::Message {
                    speaker: Speaker::Peer {
                        id,
                        nickname,
                    },
                    content,
                    at,
                });
            },
            | NodeEvent::ChatSent {
                content,
                at,
                ..
            } => {
                chat.entries.push(ChatEntry::Message {
                    speaker: Speaker::Me,
                    content,
                    at,
                });
            },
            | NodeEvent::SendFailed(reason) => {
                appended = false;
                chat.error = Some(reason);
            },
            | NodeEvent::Stopped => {
                appended = false;
                chat.stopped = true;
            },
        }
        if appended {
            chat.scroll.scroll_to_bottom();
        }
        cx.notify();
    }
}

fn notice(text: String) -> ChatEntry {
    ChatEntry::Notice {
        text,
        at: SystemTime::now(),
    }
}
