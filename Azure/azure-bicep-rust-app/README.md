# Azure Bicep Lab - SolidJS + Rust(Actix-web) 통합 애플리케이션

Azure Container Apps에 배포 가능한 통합 SolidJS + Rust(Actix-web) 애플리케이션입니다. SolidJS 프론트엔드는 빌드 시 백엔드 `wwwroot/`로 출력되어 하나의 앱으로 동작합니다.

## 의존성 업데이트 (2026-10-05)

- Rust 1.99.0과 Docker `rust:1.99.0-alpine3.24`를 사용합니다. 모든 직접 crate 요구사항과 Cargo 잠금 파일을 최신 안정 버전으로 갱신했습니다.
- 프런트엔드는 SolidJS 1.9.15, Vite 8.3.2, TypeScript 7.0.2, ESLint 10.12.0, pnpm 12.9.1을 사용합니다. TypeScript ESLint 호환 API의 병행 설치는 [프런트엔드 문서](src/frontend/README.md)에 설명되어 있습니다.
- Bicep 리소스 API는 [Microsoft 리소스 참조](https://learn.microsoft.com/en-us/azure/templates/)의 최신 안정 버전을 사용합니다. Container Apps의 2026-07-01 API와 Storage의 2026-06-01 API는 Bicep 0.47.16에 타입 정보가 없어 BCP081 경고가 발생합니다. 실제 Azure 배포 검증은 별도로 필요합니다.
- Swagger UI는 `vendored` 기능으로 패키지에 포함합니다. Docker 빌드 중 GitHub에서 UI 파일을 내려받거나 `curl`을 설치할 필요가 없습니다.

잠금 파일을 유지한 검증은 `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `pnpm install --frozen-lockfile`, `pnpm run build`, `pnpm run lint`로 실행합니다. 백엔드 통합 테스트는 메모리 SQLite에서 CRUD, UUID와 날짜 직렬화, OpenAPI 3.1 문서와 Swagger UI 응답을 확인합니다.

## 프로젝트 구조

```text
azure-bicep-rust-app/
├── infra/                          # Azure Bicep 인프라 템플릿 및 모듈
├── scripts/                        # PowerShell/Bash 빌드, 테스트, 컨테이너 관리
├── src/
│   ├── backend/                    # Rust(Actix-web) 백엔드
│   │   ├── src/                    # Rust 소스 코드
│   │   ├── Cargo.toml             # Rust 프로젝트 설정
│   │   ├── Makefile.toml          # cargo-make 태스크
│   │   ├── docs/                  # 문서
│   │   └── todos.db               # SQLite DB (로컬 개발용)
│   └── frontend/                   # SolidJS + TypeScript + Vite 프런트엔드
│       ├── src/                    # SolidJS 소스
│       ├── package.json            # Node.js 스크립트/의존성
│       └── vite.config.ts          # Vite 구성 (outDir, proxy 등)
├── azure.yaml                      # Azure Developer CLI 설정
└── README.md                       # 본 문서
```

## 빌드 및 실행

### 스크립트 실행

스크립트는 자신의 위치를 기준으로 프로젝트 경로를 계산하므로 어느 작업 디렉터리에서도 실행할 수 있습니다. Node.js와 npx, Rust/Cargo가 필요하며, 컨테이너 관리에는 Docker와 Compose가 필요합니다. pnpm은 프런트엔드의 `packageManager`에 고정된 버전을 npx로 실행합니다.

프로젝트 루트에서 PowerShell로 실행:

```powershell
./scripts/build.ps1            # 프런트엔드와 백엔드 빌드
./scripts/build.ps1 -Run       # 빌드 후 서버 실행 (8000)
./scripts/test.ps1             # 프런트엔드 린트/테스트 + Rust 테스트
./scripts/container.ps1 help
./scripts/container.ps1 status
./scripts/container.ps1 up     # 프런트엔드 빌드 후 Docker 실행 (8080)
```

Bash로 실행:

```bash
bash scripts/build.sh
bash scripts/build.sh --run
bash scripts/test.sh
bash scripts/container.sh help
bash scripts/container.sh status
bash scripts/container.sh up
```

`build`는 기본적으로 빌드만 수행하며 실행 여부를 묻지 않습니다. `test`는 서버를 계속 실행하는 대신 자동 테스트를 실행하고 종료합니다. PowerShell 스크립트는 종료 시 호출자의 작업 디렉터리를 복원합니다. Docker의 정적 파일 바인드 경로는 Compose 파일을 기준으로 해석됩니다.

### 수동 빌드(로컬 개발용)

```bash
# 1) 프런트엔드 설치 및 빌드 (출력: src/backend/wwwroot)
cd src/frontend
pnpm install
pnpm run build

# 2) 백엔드 실행 (기본 포트: http://localhost:8000)
cd ../backend
cargo run
```

### 개발 모드(HMR + 프록시)

```bash
# 터미널 1: 백엔드 실행
cd src/backend
cargo run

# 터미널 2: 프런트엔드 개발 서버 (http://localhost:5173)
cd src/frontend
pnpm run dev
```

프록시 설정은 `src/frontend/vite.config.ts`에서 `/api -> http://localhost:8000`으로 구성되어 있습니다.

## API 엔드포인트

백엔드는 TODO 관리용 REST API를 제공합니다.

- GET `/api/todos` -- TODO 목록 조회
- POST `/api/todos` -- TODO 생성
- PUT `/api/todos/{id}` -- TODO 수정
- DELETE `/api/todos/{id}` -- TODO 삭제

Swagger UI: `http://localhost:8000/swagger-ui/` (Docker에서는 8080)

## 기술 스택

### 백엔드

- Framework: Actix-web 4.9
- Language: Rust (edition 2024)
- Features:
  - REST API + SQLite (sqlx)
  - 정적 파일 서빙(`wwwroot/`) 및 SPA Fallback
  - OpenAPI(Swagger) 문서화 (utoipa + utoipa-swagger-ui)
  - UUID 기반 엔티티 식별
  - 날짜/시간 추적 (chrono)

### 프런트엔드

- Framework: SolidJS 1.9 + TypeScript
- Build Tool: Vite (Plugin: `vite-plugin-solid`)
- Package Manager: pnpm
- Features:
  - 개발 프록시(`/api -> http://localhost:8000`)
  - 프로덕션 빌드 출력: `../backend/wwwroot`

## Azure 배포(개요)

### Azure Developer CLI(azd) 초기화

```bash
mkdir ${PROJECT_NAME}
cd ${PROJECT_NAME}
azd init
```

프로젝트 루트의 `azure.yaml`을 통해 배포 구성이 관리됩니다. Bicep 템플릿은 `infra/` 디렉터리에 있습니다.

## Bicep 템플릿 빌드 예시

```bash
# Bicep 파일을 JSON으로 빌드
az bicep build --file ${PWD}/infra/main.bicep

# 빌드 결과를 stdout으로 출력
az bicep build --file ${PWD}/infra/main.bicep --stdout

# 특정 디렉토리에 빌드 결과 저장
az bicep build --file ${PWD}/infra/main.bicep --outdir ./output
```

## 시작하기

1. 저장소 클론

   ```bash
   git clone <repository-url>
   cd azure-bicep-rust-app
   ```

2. 프런트엔드 설치 및 빌드 -> 백엔드 실행

   ```bash
   cd src/frontend && pnpm install && pnpm run build
   cd ../backend && cargo run
   ```

3. 브라우저에서 확인

   - 애플리케이션: `http://localhost:8000`
   - Swagger(UI): `http://localhost:8000/swagger-ui/`

4. Azure 배포(선택)

   ```bash
   azd up
   ```
