# Kafka Playground — Rust + SolidJS

기존 `rdkafka` CLI 예제에 **SolidJS + TypeScript 웹 화면**과 **Axum API 서버**를 추가했습니다.
브라우저에서 Kafka 메시지를 발행하고 SSE(Server-Sent Events)로 실시간 수신합니다.

## 기존 코드와 확장 구조

- `src/producer.rs`: `BaseProducer`로 `localhost:9092`의 `rust` 토픽에 메시지를 발행하는 CLI입니다.
- `src/consumer.rs`: `BaseConsumer`와 `my_group_id`를 사용하는 CLI입니다.
- 기존 CLI와 빌드 설정을 유지하고, 별도의 `web` 실행 파일을 추가했습니다.
- 웹 서버는 `FutureProducer`로 **브로커가 발행을 확인한 뒤** 파티션과 오프셋을 반환합니다.
- 각 SSE 연결은 독립적인 `StreamConsumer`로 토픽의 모든 파티션을 읽습니다.

```text
SolidJS 브라우저 ── POST /api/messages ──▶ Axum ──▶ Kafka
                ◀── GET /api/events (SSE) ────────┘
```

```text
src/
├── producer.rs       # 기존 CLI 프로듀서
├── consumer.rs       # 기존 CLI 컨슈머
├── server.rs         # 웹 서버 진입점
├── web.rs            # API, Kafka 발행 및 SSE 수신
└── web_tests.rs      # librdkafka 모의 브로커 통합 테스트
frontend/
├── src/App.tsx       # SolidJS 화면과 연결 상태 관리
├── src/styles.css    # 반응형 스타일
├── vite.config.ts    # 개발용 /api 프록시
└── package.json
compose.yaml          # 로컬 Kafka(KRaft) + rust 토픽 초기화
build.rs              # 기존 네이티브 라이브러리 링크 설정
```

## 사전 준비

- Rust (`rust-toolchain.toml`의 nightly)
- Node.js 22.12 이상 및 npm
- Docker Compose와 실행 중인 Docker 데몬, 또는 기존 Kafka

Ubuntu:

```bash
sudo apt install -y build-essential cmake pkg-config libssl-dev libsasl2-dev libzstd-dev
```

macOS(Homebrew):

```bash
brew install cmake pkg-config openssl@3 cyrus-sasl zstd
```

## 빠른 실행

아래 명령은 이 프로젝트 디렉터리에서 실행합니다.

### 1. Kafka 시작

```bash
docker compose up -d
# rust 토픽 생성 완료 확인 (정상 종료 코드: 0)
docker compose logs kafka-init
```

Kafka 준비에는 시간이 걸릴 수 있습니다. `kafka-init`이 `rust` 토픽을 3개 파티션으로 생성합니다.
기존 Kafka가 `localhost:9092`에 있다면 Compose 실행을 건너뛰고 `rust` 토픽을 준비하세요.
기존 외부 설정: <https://github.com/mingyuchoo/docker-composes/tree/main/kafka>

### 2. 화면 빌드 및 서버 실행

```bash
npm ci --prefix frontend
npm run build --prefix frontend
cargo run --bin web
```

**<http://127.0.0.1:3000>** 에 접속합니다. Rust 서버가 빌드된 화면과 API를 함께 제공합니다.

1. 브로커 연결 상태와 토픽 `rust`를 확인합니다.
2. **수신 시작**을 누르고 **수신 중** 상태를 기다립니다.
3. 키와 메시지 본문을 입력하고 **메시지 발행**을 누릅니다.
4. 발행 결과와 오른쪽 수신 목록의 파티션·오프셋이 일치하는지 확인합니다.
5. 보관된 메시지도 읽으려면 수신을 중지하고 **보관된 처음부터**를 선택하여 다시 시작합니다.

### 개발 모드 (핫 리로드)

터미널 1:

```bash
cargo run --bin web
```

터미널 2:

```bash
npm ci --prefix frontend
npm run dev --prefix frontend
```

<http://127.0.0.1:5173> 에 접속합니다. Vite가 `/api`를 Rust 서버의 `127.0.0.1:3000`으로 프록시합니다.
`WEB_ADDR`를 바꾸면 `frontend/vite.config.ts`의 프록시 주소도 맞춰주세요.

### 환경 설정

| 환경변수 | 기본값 | 설명 |
| --- | --- | --- |
| `KAFKA_BROKERS` | `localhost:9092` | Kafka bootstrap 서버 목록 |
| `WEB_ADDR` | `127.0.0.1:3000` | Rust HTTP 서버 주소 |
| `FRONTEND_DIST` | `frontend/dist` | 실행 디렉터리 기준 정적 파일 경로 |

```bash
KAFKA_BROKERS=localhost:9092 WEB_ADDR=127.0.0.1:3000 cargo run --bin web
```

환경변수는 셸에서 전달합니다. `.env` 자동 로딩은 하지 않습니다.

## API

| 요청 | 역할 |
| --- | --- |
| `GET /api/health` | Kafka 메타데이터 조회로 브로커 연결 확인; 실패 시 502 |
| `POST /api/messages` | `{ "topic": "rust", "key": "hello", "payload": "안녕하세요" }` 발행 |
| `GET /api/events?topic=rust&from=latest` | 연결 시작 시점 이후의 메시지를 SSE로 수신 |
| `GET /api/events?topic=rust&from=earliest` | Kafka가 현재 보관 중인 처음부터 수신 |

```bash
# 터미널 1: ready 이벤트를 기다립니다.
curl -N 'http://127.0.0.1:3000/api/events?topic=rust&from=latest'

# 터미널 2: 발행
curl -X POST http://127.0.0.1:3000/api/messages \
  -H 'Content-Type: application/json' \
  -d '{"topic":"rust","key":"hello","payload":"SolidJS에서 Kafka까지!"}'
```

SSE 이벤트는 `ready`, `message`, `kafka-error`로 구분됩니다. 수신 데이터에는 `topic`,
`key`, `payload`, `partition`, `offset`, `timestamp`(밀리초 또는 null)가 포함됩니다.
Kafka의 64비트 오프셋은 JavaScript 정밀도 손실을 방지하기 위해 문자열로 반환합니다.

## 동작과 예제 범위

- **수신 시작 위치**: 파티션별 시작 오프셋을 조회하여 고정한 뒤 `ready`를 보냅니다.
  최신 모드에서 준비 직후 발행한 메시지도 수신 대상이 됩니다.
- **브라우저별 독립 수신**: 파티션 직접 할당을 사용합니다. consumer group의 부하 분산과
  offset commit은 사용하지 않으므로 여러 탭이 같은 메시지를 볼 수 있고 기존 CLI 그룹에는 영향을 주지 않습니다.
- **연결 종료**: 수신 중지·페이지 이탈 시 EventSource를 닫고 서버의 consumer도 해제합니다.
  오류 발생 시 자동 재접속하지 않으며, 오류를 표시하고 사용자가 다시 시작하도록 합니다.
  재시작 시 선택한 시작 위치를 다시 적용하므로 latest는 중단 중 메시지를 건너뛸 수 있고,
  earliest는 기존 메시지를 다시 보여줄 수 있습니다. 브라우저 전달 보장이나 정확히 한 번 처리는 구현하지 않습니다.
- **파티션**: 연결 시 존재하는 파티션을 읽습니다. 파티션을 늘린 뒤에는 수신을 다시 시작하세요.
  Kafka의 순서 보장은 파티션 내부에만 적용됩니다.
- **화면**: 최신 200개만 보관하며 누적 수신 건수는 별도로 표시합니다.
  ‘화면 비우기’는 Kafka 데이터를 삭제하지 않습니다. 토픽은 수신 중 변경할 수 없습니다.
- **문자열 메시지**: JSON은 일반 텍스트로 발행합니다. 키를 비우면 null key로 전송합니다.
  빈 본문도 허용하며, 외부에서 발행한 null payload(tombstone)는 구분해서 표시합니다.
  UTF-8이 아닌 바이트는 대체 문자로 표시하므로 바이너리 메시지 검사 용도는 아닙니다.
- **입력 제한**: 토픽 이름은 Kafka 이름 규칙에 따라 검증하고, 키는 UTF-8 4 KiB,
  본문은 UTF-8 256 KiB로 제한합니다. 발행 시간 초과는 성공 여부가 불확실할 수 있어 자동 재시도하지 않습니다.
- **로컬 학습용**: 인증 없는 PLAINTEXT 브로커를 전제로 합니다. 서버와 Docker 포트는 기본적으로
  localhost에 바인딩됩니다. 운영용 인증·권한·TLS·트래픽 제한은 별도로 구성해야 합니다.

추가 토픽 생성:

```bash
docker compose exec kafka /opt/kafka/bin/kafka-topics.sh \
  --bootstrap-server localhost:9092 --create --if-not-exists \
  --topic demo --partitions 3 --replication-factor 1
```

Kafka 중지(메시지는 Docker 볼륨에 보존):

```bash
docker compose down
```

## 기존 CLI 실행

```bash
cargo run --bin producer
cargo run --bin consumer
# 또는 cargo make run-producer / cargo make run-consumer
```

기존 CLI는 브로커와 토픽이 코드에 고정되어 있습니다. consumer는 기존 그룹의 커밋 오프셋과
기본 offset reset 정책을 따릅니다. 웹 수신 화면에서는 원하는 시작 위치를 선택할 수 있습니다.
`cargo run`의 기본 실행 대상도 기존 `producer`로 유지합니다.

## 검증

```bash
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
npm ci --prefix frontend
npm run build --prefix frontend
docker compose config --quiet
```

Rust 통합 테스트는 librdkafka의 `MockCluster`를 사용하므로 Docker 없이 실행할 수 있습니다.
HTTP 발행 → Kafka 프로토콜 → SSE 수신, latest/earliest 동작, 여러 브라우저의 독립 수신,
한글·줄바꿈·null key·빈 메시지, 잘못된 입력, 없는 토픽 및 브로커 장애 시 제한 시간 내 오류 응답을 검증합니다.
모의 브로커 검증은 실제 Apache Kafka 배포 검증을 대체하지 않습니다.

Docker 없이 브라우저 동작까지 확인하려면 `cargo run --example mock_broker`를 실행하고,
출력되는 `KAFKA_BROKERS=... cargo run --bin web` 명령을 다른 터미널에서 실행하세요.
프런트엔드 빌드는 동일하게 필요합니다. 이 도우미는 실제 Kafka가 아닌 일회성 모의 브로커이며,
종료하면 메시지가 사라집니다. 이후 실제 Kafka에 연결할 때는 `KAFKA_BROKERS`를 원래 주소로 되돌리세요.

`cargo make run-web`, `frontend-install`, `frontend-dev`, `frontend-build`, `kafka-up` 작업도 제공합니다.

## 참고 자료

- [SolidJS Signals](https://docs.solidjs.com/concepts/signals)
- [SolidJS onCleanup](https://docs.solidjs.com/reference/lifecycle/on-cleanup)
- [Axum SSE](https://docs.rs/axum/latest/axum/response/sse/)
- [rdkafka Consumer](https://docs.rs/rdkafka/0.39.0/rdkafka/consumer/trait.Consumer.html)
- [Apache Kafka Docker](https://kafka.apache.org/41/getting-started/docker/)
- <https://github.com/confluentinc/examples>
- <https://dev.to/abhirockzz/getting-started-with-kafka-and-rust-part-1-4hkb>
