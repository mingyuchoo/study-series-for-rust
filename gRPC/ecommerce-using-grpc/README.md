# ecommerce-using-grpc

Rust 기반 gRPC 전자상거래 상품 관리 서비스 및 Web 대시보드 모니터링 시스템입니다. Railway Oriented Programming, tracing을 사용한 구조화 로깅, 인메모리 저장소, Cargo 워크스페이스(모노레포) 아키텍처, 그리고 Axum 기반 웹 게이트웨이와 실시간 모니터링 대시보드를 적용하였습니다.

## 사전 요구사항

### Protocol Buffers 컴파일러

**Ubuntu**
```bash
sudo apt install protobuf-compiler
```

**Fedora**
```bash
sudo dnf install protobuf-compiler
```

**macOS**
```bash
brew install protobuf
```

## 프로젝트 구조

Cargo 워크스페이스(모노레포) 구조입니다:

```
ecommerce-using-grpc/
├── Cargo.toml          # 워크스페이스 설정
├── crates/
│   ├── proto/          # 공유 Protocol Buffer 정의
│   │   ├── proto/
│   │   │   └── ProductInfo.proto
│   │   ├── build.rs
│   │   └── src/lib.rs
│   ├── server/         # gRPC 서버 및 Health Check 구현
│   │   └── src/
│   │       ├── lib.rs  # 서비스 로직, 인메모리 저장소, 에러 처리
│   │       └── main.rs # 서버 바이너리 & tonic-health 등록
│   ├── client/         # gRPC 클라이언트 CLI 구현
│   │   └── src/
│   │       └── main.rs # 클라이언트 바이너리
│   ├── web/            # Axum 기반 Web 게이트웨이 & 대시보드
│   │   └── src/
│   │       ├── index.html # 반응형 실시간 대시보드 UI
│   │       └── main.rs    # REST API & 모니터링 엔드포인트
│   └── tests/          # 통합 테스트
│       └── tests/
│           └── product_service_test.rs
├── proto/              # 원본 proto 파일 (참조용)
│   └── ProductInfo.proto
└── scripts/
    └── run.sh          # 올인원 실행 및 관리 스크립트
```

## 주요 기능

- **Cargo 워크스페이스(모노레포) 아키텍처** - 여러 크레이트로 체계적 구성
- **gRPC 기반 상품 관리 서비스** - 고성능 클라이언트-서버 통신
- **표준 gRPC Health Check** - `tonic-health` 표준 프로토콜 적용
- **웹 대시보드 & 모니터링** - 브라우저 기반 실시간 상태 관측 및 상품 관리 (Axum & Tailwind CSS)
  - gRPC 서버 상태 실시간 표시 (`SERVING` / `DISCONNECTED`)
  - 실시간 Ping 지연시간 (Latency RTT ms) 측정
  - 총 상품 등록 수 및 가동 시간 (Uptime) 표시
  - 실시간 상품 등록 / 조회 / 목록 확인
- **인메모리 저장소** - `Arc<Mutex<>>` 기반 스레드 안전 `HashMap`
- **자동 증가 ID** - 서버가 atomic 카운터로 상품 ID 할당
- **Railway Oriented Programming** - 깔끔한 에러 처리 패턴
- **tracing을 사용한 구조화 로깅** - 프로덕션 수준의 관측 가능성
- **입력 유효성 검사** - 상품명(비어있지 않음) 및 가격(양수) 검증

## 주요 의존성

| 패키지 | 버전 | 설명 |
|--------|------|------|
| `tonic` | 0.14.5 | gRPC 프레임워크 |
| `tonic-health` | 0.14.5 | gRPC 표준 헬스체크 |
| `axum` | 0.8.9 | 웹 서버 및 REST API 게이트웨이 |
| `prost` | 0.14.3 | Protocol Buffers 구현 |
| `tokio` | 1.50.0 | 비동기 런타임 |
| `anyhow` | 1.0.102 | 에러 처리 |
| `thiserror` | 2.0.18 | 커스텀 에러 타입 |
| `tracing` | 0.1.44 | 구조화 로깅 |
| `tracing-subscriber` | 0.3.22 | 로그 출력 |

## 실행 가이드

### 1. 스크립트로 간편 실행 (추천)

```bash
# 전체 파이프라인 (검사 + 포맷 + 린트 + 빌드 + 테스트 + gRPC 및 Web 서버 지속 실행)
./scripts/run.sh

# gRPC 서버 백그라운드 기동 + Web 대시보드 바로 실행 (http://localhost:3000, 지속 실행)
./scripts/run.sh web

# 서버 백그라운드 기동 + CLI 클라이언트 1회 테스트 후 자동 종료
./scripts/run.sh both

# CI용 비대화형 파이프라인 검증 후 자동 종료
./scripts/run.sh ci
```

### 2. 개별 서비스 수동 실행

```bash
# 1) gRPC 서버 시작 (포트: [::1]:50051)
cargo run -p server

# 2) Web 대시보드 시작 (포트: 0.0.0.0:3000 -> 브라우저 접속 http://localhost:3000)
cargo run -p web

# 3) CLI 클라이언트 테스트 실행
cargo run -p client
```

## 테스트

```bash
# 전체 워크스페이스 테스트
cargo test

# 특정 크레이트 테스트
cargo test -p tests
```

## gRPC RPC 정의

### AddProduct
새 상품을 추가합니다. 서버가 자동으로 고유 ID를 할당합니다.
- **요청:** `Product (name, description, price)`
- **응답:** `ProductId (id)`

### GetProduct
ID로 상품 정보를 조회합니다.
- **요청:** `ProductId (id)`
- **응답:** `Product (id, name, description, price)`

### ListProducts
등록된 전체 상품 목록을 반환합니다.
- **요청:** `Empty`
- **응답:** `ProductList (repeated Product products)`

## Web REST API 엔드포인트

- `GET /` : 웹 대시보드 UI (HTML)
- `GET /api/health` : gRPC 서버 헬스체크, RTT 지연시간, 가동시간 조회
- `GET /api/products` : 전체 상품 목록 조회
- `GET /api/products/:id` : 상품 단건 조회
- `POST /api/products` : 상품 등록 (`{ "name": "...", "description": "...", "price": 100.0 }`)

## License

프로젝트 라이선스 파일을 참조하세요.
