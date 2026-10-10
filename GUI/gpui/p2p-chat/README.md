# P2P Chat (Rust + gpui)

중앙 서버 없이 같은 네트워크의 PC끼리 직접 대화하는 데스크톱 채팅 앱입니다. UI는 Zed의 GPU UI 프레임워크인 [gpui](https://crates.io/crates/gpui)로 만들었습니다.

- **피어 발견**: 각 노드가 HELLO를 UDP 멀티캐스트(`239.255.42.99:50050`)로 2초마다 보냅니다.
- **메시지 전송**: 연결된 모든 피어에게 UDP 유니캐스트로 보냅니다.
- **퇴장**: 창을 닫으면 GOODBYE를 보내고, 6초 동안 응답이 없는 피어는 "응답 없음"으로 처리합니다.

---

## 1. 구조 (Cargo Workspace + Clean Architecture)

기존 구조에서는 `p2p_app`이 `p2p_infra`에 직접 의존했고, 피어/채팅 규칙과
스레드·소켓·타이머·채널 처리가 같은 모듈에 있었다. 닉네임 검증도 GUI에만
있었다. 이제 순수한 상태 전이와 부수효과 실행을 분리하고, 바깥 계층이
안쪽 계층의 계약을 구현하도록 의존 방향을 바꿨다.

```text
p2p-chat/
├── Cargo.toml                       # 멤버, 공통 패키지 정보/의존성
├── Makefile.toml
├── scripts/run.ps1 / run.sh
└── crates/
    ├── p2p_core/                    # 순수한 도메인과 메시지 변환
    │   └── src/
    │       ├── domain.rs            # PeerTable, PeerEvent, ChatEntry
    │       ├── message.rs           # Message, JSON 규약, MessageError
    │       ├── domain_tests.rs
    │       └── message_tests.rs
    ├── p2p_app/                     # 순수한 유스케이스와 경계 계약
    │   └── src/
    │       ├── config.rs            # NodeConfig, 공통 입력 검증
    │       ├── events.rs            # NodeCommand, NodeEvent, PeerInfo
    │       ├── node.rs              # 수신/만료/전송 계획과 결과 판단
    │       ├── ports.rs             # NetworkPort, Inbound
    │       └── tests.rs             # 시간/네트워크 대기 없는 테스트
    ├── p2p_infra/                   # NetworkPort의 UDP 구현
    │   └── src/lib.rs               # 소켓, 수신 태스크, NetworkError
    ├── p2p_runtime/                 # 실행 부수효과와 의존성 조립
    │   ├── src/
    │   │   ├── lib.rs               # start, NodeSession, StartError, UUID/스레드
    │   │   └── runner.rs            # Tokio 타이머, 채널, 포트 호출
    │   └── tests/
    │       ├── startup.rs           # 설정 오류와 포트 충돌
    │       └── two_nodes.rs         # 실제 UDP 노드 간 통합 테스트
    └── p2p_gui/                     # GPUI 화면과 사용자 입력
        └── src/
            ├── main.rs             # 창, 단축키, 종료 처리
            ├── app.rs              # 세션 시작, 이벤트/GPUI 연결
            ├── model.rs            # 순수한 대화 화면 상태 전이
            ├── view.rs             # 렌더링
            └── text_input.rs       # 한글 IME 입력창
```

다른 Workspace 크레이트에 대한 의존성은 다음과 같다. 순환 의존성은 없다.

| 크레이트 | 의존하는 크레이트 | 책임 |
| --- | --- | --- |
| `p2p_core` | 없음 | 도메인 상태와 결정적인 메시지 변환 |
| `p2p_app` | `p2p_core` | 비즈니스 판단, 입력 검증, 포트 계약 |
| `p2p_infra` | `p2p_app`, `p2p_core` | UDP 어댑터 |
| `p2p_runtime` | `p2p_app`, `p2p_core`, `p2p_infra` | 부수효과 실행과 조립 |
| `p2p_gui` | `p2p_app`, `p2p_core`, `p2p_runtime` | 화면과 실행 경계 연결 |

`p2p_core`와 `p2p_app`은 소켓, 스레드, 채널, Tokio, GPUI, UUID 생성에
의존하지 않는다. JSON 직렬화는 I/O가 없는 순수한 변환이므로 `message.rs`에
격리했고, 기존 HELLO/CHAT/GOODBYE 와이어 형식과 4096바이트 제한을 유지한다.
I/O 오류는 `NetworkError`와 `StartError`로 바깥 계층이 소유한다.

- `Node::receive`와 `Node::expire`는 호출자가 전달한 시각으로만 상태를 바꾼다.
  피어 테이블은 ID 순서로 조회/만료되어 결과 순서도 결정적이다.
- `Node::prepare_chat`은 내용과 크기를 검증하고 전송 대상을 정한다.
  `ChatDispatch::finish`는 전송 결과를 받아 이벤트를 만든다. 일부 피어에게만
  성공해도 기존처럼 `ChatSent`와 성공한 피어 수를 반환한다.
- `NetworkPort`는 애플리케이션이 소유하고 UDP 어댑터가 구현한다.
  오류는 연관 타입으로 받아 구체적인 소켓 오류가 안쪽으로 새지 않는다.
- GUI의 `Conversation::apply`도 시각을 입력받는다. 스크롤/화면 갱신은
  `app.rs`가 담당하므로 피어 갱신은 대화 기록과 읽던 위치를 바꾸지 않는다.
- 닉네임/포트 검증은 `NodeConfig::new`에 모았다. `Node::new`도 재검증하므로
  GUI를 통하지 않는 호출도 동일한 규칙을 따른다.

노드는 전용 스레드의 Tokio 런타임에서 돌고, GUI와는 두 채널로 통신한다.

- GUI → 노드: `NodeCommand::{SendChat, Shutdown}`
- 노드 → GUI: `NodeEvent::{PeerJoined, PeerUpdated, PeerLeft, ChatReceived, ChatSent, SendFailed, Stopped}`

라이브러리 사용 시 실행 진입점은 `p2p_runtime::start`다.
기존 `p2p_app::start`/`NodeSession`은 `p2p_runtime`으로,
`p2p_core::NetworkPort`는 `p2p_app`으로 옮겼다.
기존 `P2PError` 대신 변환 오류는 `MessageError`, 전송 오류는 `NetworkError`,
시작 오류는 `StartError`를 사용한다.

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

```sh
# GUI와 실제 네트워크 없이 도메인/유스케이스 검증
cargo test -p p2p_core -p p2p_app

# 전체 테스트 (실제 멀티캐스트와 GUI 모델 테스트 포함)
cargo test --workspace

# 공통 품질 검사
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

- `p2p_core`: 피어 등록/만료/결정적 순서, 메시지 형식, 한글 왕복, 크기 제한,
  잘못된 JSON 거부.
- `p2p_app`: 공통 설정 검증, 자기 메시지 무시, 피어 갱신/퇴장, 주입한 시각과
  타임아웃 경계, 전송 대상과 부분 성공/전체 실패.
- `p2p_runtime`: 가짜 `NetworkPort`로 채널/전송/종료 흐름을 검증하고,
  시작 시 잘못된 설정과 포트 충돌도 확인한다.
- `p2p_runtime/tests/two_nodes.rs`: 실제 두 노드가 멀티캐스트로 발견하고
  채팅/GOODBYE를 주고받는다. 루프백 멀티캐스트가 막힌 환경에서는 실패할 수 있다.
- `p2p_gui`: GPUI 창을 열지 않고 순수한 대화 모델의 기록/피어/오류/종료 전이를
  검증한다. 이 크레이트를 컴파일하려면 해당 플랫폼의 GPUI 빌드 환경은 필요하다.

---

## 5. 알려진 제약

- **UDP 특성**: 재전송과 순서 보장이 없고 전달 확인도 없습니다. 메시지가 유실될 수 있습니다.
- **메시지 크기**: 메시지 하나는 데이터그램 한 개(4096바이트, JSON 포함)에 들어가야 합니다. 초과하면 "메시지가 너무 큽니다" 오류가 표시됩니다.
- **대화 기록**: 메모리에만 있어서 앱을 종료하면 사라집니다.
- **대화 범위**: 1:1 대화는 없고, 메시지는 발견된 모든 피어에게 같은 내용으로 전송됩니다.
- **네트워크 조건**: 같은 LAN에서 멀티캐스트가 통해야 발견됩니다. 방화벽이 UDP `50050`과 수신 포트를 막으면 발견되지 않습니다.
- **플랫폼**: Windows에서 주로 확인했습니다.
