# P2P Chat (Rust + gpui)

중앙 서버 없이 같은 네트워크의 PC끼리 직접 대화하는 데스크톱 채팅 앱입니다. UI는 Zed의 GPU UI 프레임워크인 [gpui](https://crates.io/crates/gpui)로 만들었습니다.

- **피어 발견**: 각 노드가 HELLO를 UDP 멀티캐스트(`239.255.42.99:50050`)로 2초마다 보냅니다.
- **메시지 전송**: 연결된 모든 피어에게 UDP 유니캐스트로 보냅니다.
- **퇴장**: 창을 닫으면 GOODBYE를 보내고, 6초 동안 응답이 없는 피어는 "응답 없음"으로 처리합니다.

---

## 1. 구조 (Clean Architecture)

```text
p2p-demo/
├── Cargo.toml                       # 워크스페이스 루트
├── Makefile.toml
├── scripts/
│   └── run.ps1 / run.sh             # fmt → clippy → test → build → 앱 실행
└── crates/
    ├── p2p_core/                    # [Domain] 순수 로직 (I/O 없음)
    │   └── src/
    │       ├── domain.rs            # Message, Peer, PeerTable, ChatEntry
    │       ├── domain_tests.rs      # 단위 테스트
    │       └── lib.rs               # NetworkPort 포트 트레이트, P2PError
    ├── p2p_infra/                   # [Infrastructure] UDP 소켓 어댑터
    │   └── src/lib.rs               # 멀티캐스트/유니캐스트 송수신, 수신 채널
    ├── p2p_app/                     # [Application] 노드 실행 루프와 UI 경계
    │   ├── src/lib.rs               # start(), NodeCommand, NodeEvent
    │   └── tests/two_nodes.rs       # 두 노드 통합 테스트
    └── p2p_gui/                     # [Presentation] gpui 데스크톱 앱
        └── src/
            ├── main.rs              # 창, 단축키, 종료 처리
            ├── app.rs               # 상태: 설정 → 노드 시작 → 이벤트 반영
            ├── view.rs              # gpui 제목 표시줄, 설정 화면과 채팅 화면 렌더링
            └── text_input.rs        # 한글 IME를 지원하는 입력창
```

의존 방향은 `p2p_gui → p2p_app → p2p_infra → p2p_core`입니다. `p2p_core`는 다른 크레이트에 의존하지 않습니다.

노드는 전용 스레드(자체 tokio 런타임)에서 돌고, GUI와는 두 개의 채널로만 통신합니다.
- GUI → 노드: `NodeCommand::{SendChat, Shutdown}`
- 노드 → GUI: `NodeEvent::{PeerJoined, PeerUpdated, PeerLeft, ChatReceived, ChatSent, SendFailed, Stopped}`

---

## 2. 실행

```powershell
# 검증(fmt, clippy, test) 후 앱 실행
.\scripts\run.ps1

# 검증만 수행
.\scripts\run.ps1 -SkipRun
```

Bash 환경에서는 `./scripts/run.sh`, 또는 바로 `cargo run -p p2p_gui`로 실행할 수 있습니다.

같은 PC에서 앱을 여러 개 띄우려면 창마다 **수신 포트를 다르게** 입력하세요 (예: 9001, 9002).

---

## 3. 사용법

1. **설정 화면**: 닉네임(최대 20자)과 UDP 수신 포트를 입력하고 Enter 또는 **시작**을 누릅니다.
2. **왼쪽 사이드바**: 발견된 피어 목록(닉네임, IP:포트)과 내 닉네임, 노드 ID 앞 8자리를 보여 줍니다.
3. **대화 영역**: 내 메시지는 오른쪽, 상대 메시지는 왼쪽에 표시됩니다. 입장, 퇴장, 응답 없음은 가운데 공지로 나타납니다.
4. **메시지 전송**: 입력창에 글을 쓰고 Enter를 누르면 모든 피어에게 전송됩니다.
5. **종료**: 창을 닫거나 `Ctrl+Q`를 누르면 GOODBYE를 보낸 뒤 종료합니다.

---

## 4. 테스트

```powershell
cargo test --workspace
```

- `p2p_core`: 메시지 직렬화와 역직렬화, 한글 왕복, 데이터그램 크기 제한, PeerTable 상태 전이
- `p2p_app/tests/two_nodes.rs`: 두 노드가 멀티캐스트로 서로를 발견하고, 채팅을 주고받고, GOODBYE로 퇴장하는지 확인합니다. 루프백 멀티캐스트가 막힌 환경에서는 실패할 수 있습니다.

---

## 5. 알려진 제약

- **UDP 특성**: 재전송과 순서 보장이 없고 전달 확인도 없습니다. 메시지가 유실될 수 있습니다.
- **메시지 크기**: 메시지 하나는 데이터그램 한 개(4096바이트, JSON 포함)에 들어가야 합니다. 초과하면 "메시지가 너무 큽니다" 오류가 표시됩니다.
- **대화 기록**: 메모리에만 있어서 앱을 종료하면 사라집니다.
- **대화 범위**: 1:1 대화는 없고, 메시지는 발견된 모든 피어에게 같은 내용으로 전송됩니다.
- **네트워크 조건**: 같은 LAN에서 멀티캐스트가 통해야 발견됩니다. 방화벽이 UDP `50050`과 수신 포트를 막으면 발견되지 않습니다.
- **플랫폼**: Windows에서 주로 확인했습니다.
