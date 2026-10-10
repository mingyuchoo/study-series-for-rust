# P2P Chat (Rust + gpui)

중앙 서버 없이 같은 네트워크의 PC끼리 직접 대화하는 데스크톱 채팅 앱입니다. UI는 Zed의 GPU UI 프레임워크인 [gpui](https://crates.io/crates/gpui)로 만들었습니다.

- **피어 발견**: 각 노드가 HELLO를 UDP 멀티캐스트(`239.255.42.99:50050`)로 2초마다 보냅니다.
- **메시지 전송**: 발견된 모든 피어에게 Quinn 기반 QUIC 스트림으로 보냅니다. 상호 TLS 인증서를 검증하며, 수신 큐 등록 ACK를 기다립니다.
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
    ├── p2p_infra/                   # UDP 발견 + QUIC 전송 구현
    │   └── src/
    │       ├── lib.rs               # 어댑터 조립, 공개 API, NetworkError
    │       ├── protocol.rs          # 순수한 발견 규약/채팅 검증, 인증서 지문
    │       ├── discovery.rs         # UDP 멀티캐스트 발견과 피어 갱신
    │       ├── quic.rs              # 상호 TLS, 연결 재사용, 프레임/ACK I/O
    │       ├── identity.rs          # 키·인증서 저장과 인증서 지문 고정
    │       └── tests.rs             # 실제 QUIC, 인증 거부, 프레임/타임아웃
    ├── p2p_runtime/                 # 실행 부수효과와 의존성 조립
    │   ├── src/
    │   │   ├── lib.rs               # start, NodeSession, StartError, 저장된 신원/스레드
    │   │   └── runner.rs            # Tokio 타이머, 채널, 포트 호출
    │   └── tests/
    │       ├── startup.rs           # 설정 오류와 포트 충돌
    │       └── two_nodes.rs         # 실제 QUIC 노드 간 통합 테스트
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
| `p2p_infra` | `p2p_app`, `p2p_core` | UDP 발견 + QUIC 어댑터 |
| `p2p_runtime` | `p2p_app`, `p2p_core`, `p2p_infra` | 부수효과 실행과 조립 |
| `p2p_gui` | `p2p_app`, `p2p_core`, `p2p_runtime` | 화면과 실행 경계 연결 |

`p2p_core`와 `p2p_app`은 소켓, 스레드, 채널, Tokio, GPUI, UUID 생성에
의존하지 않는다. JSON 직렬화는 I/O가 없는 순수한 변환이므로 `message.rs`에
격리했고, CHAT JSON과 4096바이트 제한을 유지한다. 발견 메시지는 프로토콜 버전과 인증서를 추가한 HELLO/GOODBYE JSON이다.
I/O 오류는 `NetworkError`와 `StartError`로 바깥 계층이 소유한다.

인프라 안에서도 순수한 전송 규약과 부수효과를 분리한다. `protocol.rs`는 입력
바이트와 인증된 노드 ID만 받아 발견 데이터/채팅을 변환·검증하고 인증서의
지문을 계산한다. 소켓, 파일, 현재 시각, 비동기 런타임을 사용하지 않으므로
네트워크 없이 경계값을 테스트할 수 있다. UDP 처리는 `discovery.rs`, QUIC
연결과 스트림 처리는 `quic.rs`, 파일 저장은 `identity.rs`가 맡는다. 이 모듈은
크레이트 밖에 노출하지 않고 기존 `QuicNetworkAdapter`와 `NetworkPort`로만
연결한다. 발견과 QUIC이 공유하는 피어/신뢰 상태의 소유권도 어댑터에 유지한다.

- `Node::receive`와 `Node::expire`는 호출자가 전달한 시각으로만 상태를 바꾼다.
  피어 테이블은 ID 순서로 조회/만료되어 결과 순서도 결정적이다.
- `Node::prepare_chat`은 내용과 크기를 검증하고 전송 대상을 정한다.
  `ChatDispatch::finish`는 전송 결과를 받아 이벤트를 만든다. 일부 피어에게만
  성공해도 기존처럼 `ChatSent`와 성공한 피어 수를 반환한다.
- `NetworkPort`는 애플리케이션이 소유하고 QUIC 어댑터가 구현한다.
  오류는 연관 타입으로 받아 구체적인 소켓 오류가 안쪽으로 새지 않는다.
- GUI의 `Conversation::apply`도 시각을 입력받는다. 스크롤/화면 갱신은
  `app.rs`가 담당하므로 피어 갱신은 대화 기록과 읽던 위치를 바꾸지 않는다.
- 닉네임/포트 검증은 `NodeConfig::new`에 모았다. `Node::new`도 재검증하므로
  GUI를 통하지 않는 호출도 동일한 규칙을 따른다.

노드는 전용 스레드의 Tokio 런타임에서 돌고, GUI와는 두 채널로 통신한다.
채팅은 32개 대기열의 별도 전송 작업에서 처리한다. 같은 메시지는 peer들에게
동시에 전송하며, 연결·전송·ACK 대기는 peer마다 최대 3초다. 그동안 발견,
수신, 만료 처리는 계속된다. 전송 작업은 메시지 순서를 유지하므로 느린 peer가
있으면 메시지마다 최대 3초를 기다리고, 대기열이 쌓이면 지연이 누적된다. 종료 시에는 최대
3초 동안 대기한 뒤 남은 메시지를 취소하고 `SendFailed`로 알린다.

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

같은 PC에서 앱을 여러 개 띄우려면 창마다 **QUIC 수신 포트를 다르게** 입력하세요 (예: 9001, 9002).

---

## 3. 사용법

1. **설정 화면**: 닉네임(최대 20자)과 QUIC 채팅 포트를 입력하고 Enter 또는 **시작**을 누릅니다.
2. **왼쪽 사이드바**: 발견된 피어 목록(닉네임, IP:포트)과 내 닉네임, 인증서 SHA-256 지문 기반 노드 ID 앞 8자리를 보여 줍니다.
3. **대화 영역**: 내 메시지는 오른쪽, 상대 메시지는 왼쪽에 표시됩니다. 입장, 퇴장, 응답 없음은 가운데 공지로 나타납니다.
4. **메시지 전송**: 입력창에 글을 쓰고 Enter를 누르면 모든 피어에게 전송됩니다.
5. **종료**: 창을 닫거나 `Ctrl+Q`를 누르면 GOODBYE를 보낸 뒤 종료합니다.

---

## 4. 테스트

```sh
# GUI와 실제 네트워크 없이 도메인/유스케이스 검증
cargo test -p p2p_core -p p2p_app

# 전체 테스트 (실제 멀티캐스트, QUIC, GUI 모델 테스트 포함)
cargo test --workspace

# 공통 품질 검사
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

- `p2p_core`: 피어 등록/만료/결정적 순서, 메시지 형식, 한글 왕복, 크기 제한,
  잘못된 JSON 거부.
- `p2p_app`: 공통 설정 검증, 자기 메시지 무시, 피어 갱신/퇴장, 주입한 시각과
  타임아웃 경계, 전송 대상과 부분 성공/전체 실패.
- `p2p_infra`: 순수한 프로토콜 테스트로 발견 JSON 규약, 버전·인증서·크기 제한,
  프레임 길이 경계값, 인증된 채팅 발신자와 빈 메시지 거부를 검증한다. 실제
  네트워크 테스트는 상호 TLS 연결, 양방향 한글 채팅·순서·연결 재사용, 잘못된
  서버/클라이언트 인증서, 발신자 위조·과대 프레임, UDP 채팅 거부, 타임아웃,
  키·지문 저장을 검증한다.
- `p2p_runtime`: 가짜 `NetworkPort`로 채널/전송/종료 흐름과 느린 peer 중 수신·발견·타이머 처리, 종료 시 전송 취소를 검증하고,
  시작 시 잘못된 설정과 포트 충돌도 확인한다.
- `p2p_runtime/tests/two_nodes.rs`: 실제 두 노드가 멀티캐스트로 발견하고
  QUIC 채팅/GOODBYE를 주고받고, 같은 신원으로 재시작해 재연결한다. 응답하지 않는 QUIC peer가 있어도 실제 노드의 수신 처리가 계속되는지 검증한다. 루프백 멀티캐스트가 막힌 환경에서는 실패할 수 있다.
- `p2p_gui`: GPUI 창을 열지 않고 순수한 대화 모델의 기록/피어/오류/종료 전이를
  검증한다. 이 크레이트를 컴파일하려면 해당 플랫폼의 GPUI 빌드 환경은 필요하다.

---

## 5. 알려진 제약

- **전송 결과**: QUIC이 패킷 손실을 복구하고 같은 스트림의 순서를 유지합니다. `ChatSent`는 상대 노드의 수신 큐 등록 ACK를 받은 peer 수를 뜻하며, 사용자의 읽음이나 디스크 저장을 뜻하지 않습니다. ACK 전에 연결이 끊기면 실제 수신 여부는 불확실하므로 자동 재전송하지 않습니다.
- **메시지 크기**: 채팅 JSON과 발견 데이터그램은 각각 최대 4096바이트입니다. 채팅은 길이 접두어가 있는 QUIC 스트림으로 전송합니다. 초과하면 "메시지가 너무 큽니다" 오류가 표시됩니다.
- **대화 기록**: 메모리에만 있어서 앱을 종료하면 사라집니다.
- **대화 범위**: 1:1 대화는 없고, 메시지는 발견된 모든 피어에게 같은 내용으로 전송됩니다.
- **네트워크 조건**: 같은 LAN에서 멀티캐스트가 통해야 발견됩니다. 방화벽이 UDP `50050`과 수신 포트를 막으면 발견되지 않습니다.
- **플랫폼**: 기존 Windows 실행 환경에 더해 macOS에서 QUIC 통합 테스트를 검증합니다.

## 6. 인증서와 신뢰 정책

- Quinn/rustls의 기본 인증서·TLS 서명 검증을 사용하며, 검증을 생략하는 verifier는 없다. ALPN은 `p2p-chat/1`이다. 양쪽 모두 발견한 인증서를 검증하고, 메시지의 `node_id`가 연결에서 인증된 인증서의 지문과 일치해야 채팅을 받아들인다.
- 자체 서명 키·인증서는 사용자 데이터 디렉터리의 `p2p-chat/<포트>/identity.json`에 저장한다. macOS는 `~/Library/Application Support`, Windows는 `%LOCALAPPDATA%`, Linux는 `$XDG_DATA_HOME` 또는 `~/.local/share` 아래다. Unix에서 키 파일 권한은 `0600`이다. 재시작하면 같은 키와 노드 ID를 사용하며 닉네임은 신뢰 식별자가 아니다.
- TOFU(최초 연결 신뢰) 방식이다. 성공한 TLS 연결에서 확인한 IP:포트와 인증서 지문은 같은 디렉터리의 `trusted-peers.json`에 저장한다. 이미 저장한 주소의 인증서가 바뀌면 발견·연결을 거부한다. 손상된 상태 파일은 조용히 초기화하지 않고 시작을 실패시킨다.
- 최초 발견/연결은 사전 인증된 신원이 아니므로 최초 접속의 중간자 공격을 막지 못한다. UDP HELLO/GOODBYE는 암호화·서명되지 않아 발견 정보와 퇴장 알림은 위조·재전송될 수 있다. 채팅 내용은 검증된 상호 TLS 연결에서만 받아들인다.
- 인증서를 재발급하거나 기존 IP:포트를 다른 장비가 사용하는 경우, 상대 신원을 별도로 확인한 뒤 해당 주소의 저장된 지문을 수동으로 제거해야 한다. 키 파일을 삭제하면 새 신원이 생성되므로 상대의 기존 신뢰와 충돌한다.
- 기존 UDP 채팅 버전과는 호환되지 않는다. 같은 LAN의 모든 노드를 함께 업데이트해야 한다. QUIC도 UDP를 사용하므로 방화벽에서 UDP `50050`과 선택한 채팅 포트를 허용해야 한다.
- 테스트는 `start_with_state_dir`와 임시 디렉터리를 사용해 실제 사용자 키·신뢰 파일과 분리한다.
