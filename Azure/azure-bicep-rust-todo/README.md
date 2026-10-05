# Azure Bicep Rust TODO

SolidJS + TypeScript 프런트엔드와 Rust + Actix-web 백엔드로 구성한 TODO 관리 예제입니다. TODO는 SQLite에 저장하며, 프런트엔드 빌드 결과를 백엔드가 정적 파일로 제공합니다. Azure Container Apps용 Bicep 템플릿과 로컬 빌드·테스트·Docker 관리 스크립트를 포함합니다.

## 프로젝트 구조

```text
azure-bicep-rust-todo/
├── infra/                       # 구독 범위 Bicep 템플릿과 리소스 모듈
├── scripts/                     # PowerShell / Bash 공통 작업 스크립트
├── src/
│   ├── .node-version            # Node.js v24.2.0
│   ├── .dockerignore            # Docker 빌드 컨텍스트 제외 항목
│   ├── backend/                 # Actix-web API, SQLite, 정적 파일 제공
│   │   ├── src/                 # domain / application / adapters / infrastructure
│   │   ├── tests/api.rs         # 메모리 SQLite 기반 API 통합 테스트
│   │   ├── docs/ARCHITECTURE.md
│   │   └── wwwroot/             # 프런트엔드 빌드 산출물 (Git 제외)
│   ├── frontend/                # SolidJS UI, Vite, Vitest
│   └── docker/                  # Dockerfile, docker-compose.yml
├── azure.yaml                   # Azure Developer CLI 서비스 설정
└── README.md
```

세부 설정은 [백엔드 README](src/backend/README.md)와 [프런트엔드 README](src/frontend/README.md)를 참고하세요.

## 개발 환경

다음 버전은 저장소 설정에 선언된 값입니다. 의존성의 정확한 해석 결과는 `Cargo.lock`과 `pnpm-lock.yaml`을 기준으로 합니다.

| 항목 | 저장소 설정 |
| --- | --- |
| Rust | `1.99.0`, edition `2024` (`src/backend/rust-toolchain.toml`) |
| Node.js | `.node-version`: `v24.2.0`; 프런트엔드 engines: `^20.19.0 \|\| ^22.13.0 \|\| >=24` |
| pnpm | `12.9.1` (`packageManager`) |
| 백엔드 주요 의존성 | Actix-web `4.15.0`, SQLx `0.9.0`, utoipa `6.0.0` |
| 프런트엔드 주요 의존성 | SolidJS `^1.9.15`, Vite `^8.3.2`, TypeScript `^7.0.2` |
| Docker 빌드 이미지 | `rust:1.99.0-alpine3.24` |

로컬 실행에는 Rust/Cargo와 Node.js/npm의 `npx`가 필요합니다. 스크립트는 `npx`로 지정된 pnpm 버전을 실행하므로 pnpm 전역 설치가 필요하지 않습니다. Docker 실행에는 Docker 엔진과 Compose가 추가로 필요합니다.

## 빠른 시작

이 프로젝트 디렉터리에서 실행합니다.

PowerShell:

```powershell
./scripts/build.ps1 -Run
```

Bash:

```bash
bash scripts/build.sh --run
```

프런트엔드를 잠금 파일 기준으로 설치·빌드하고, 백엔드를 개발 프로필로 빌드한 뒤 실행합니다. `-Run` / `--run`을 생략하면 빌드 후 종료합니다. 스크립트는 자신의 위치를 기준으로 경로를 계산하며, PowerShell 스크립트는 호출자의 작업 디렉터리를 복원합니다.

| 접속 대상 | 주소 |
| --- | --- |
| 통합 애플리케이션 | <http://localhost:8000> |
| TODO API | <http://localhost:8000/api/todos> |
| Swagger UI | <http://localhost:8000/swagger-ui/> |
| OpenAPI JSON | <http://localhost:8000/api-docs/openapi.json> |

수동으로 실행하려면 다음 순서를 따릅니다.

```bash
cd src/frontend
npx --yes pnpm@12.9.1 install --frozen-lockfile
npx --yes pnpm@12.9.1 run build
cd ../backend
cargo run --locked
```

빌드 결과는 `src/backend/wwwroot/`에 생성됩니다. 서버는 **프로세스 작업 디렉터리**의 `wwwroot/`와 `todos.db`를 사용하므로 백엔드 디렉터리에서 실행하세요. DB와 `todos` 테이블은 없으면 생성합니다. 임의의 경로를 `index.html`로 돌려주는 SPA fallback은 구현되어 있지 않습니다.

## 프런트엔드 개발 서버

터미널 1:

```bash
cd src/backend
cargo run --locked
```

터미널 2:

```bash
cd src/frontend
npx --yes pnpm@12.9.1 install --frozen-lockfile
npx --yes pnpm@12.9.1 run dev
```

Vite 개발 서버에서 UI를 확인합니다(기본 <http://localhost:5173>, 실제 주소는 터미널 출력 확인). `/api`로 시작하는 요청은 `http://localhost:8000`으로 전달됩니다. 백엔드 포트를 변경하면 [Vite 프록시 설정](src/frontend/vite.config.ts)도 맞춰야 합니다. Swagger UI는 백엔드 주소로 직접 접속합니다.

## API

| 메서드 | 경로 | 동작 | 성공 응답 |
| --- | --- | --- | --- |
| GET | `/api/todos` | 생성일 내림차순 목록 조회 | `200`, TODO 배열 |
| POST | `/api/todos` | 제목과 선택적 설명으로 생성 | `201`, 생성된 TODO |
| PUT | `/api/todos/{id}` | 전달한 제목·설명·완료 상태 수정 | `200`, 수정된 TODO |
| DELETE | `/api/todos/{id}` | 삭제 | `204`, 본문 없음 |

TODO에는 UUID v4 문자열 ID, 제목, 선택적 설명, 완료 여부, UTC 생성·수정 시각이 포함됩니다. API 인증은 구현되어 있지 않습니다. 요청 예제와 오류 응답은 [백엔드 README](src/backend/README.md#api)를 참고하세요.

## 검증 명령

```powershell
./scripts/test.ps1
```

```bash
bash scripts/test.sh
```

테스트 스크립트는 프런트엔드 의존성 설치(`--frozen-lockfile`), ESLint, Vitest, `cargo test --locked` 순서로 실행하고 종료합니다. 프런트엔드 프로덕션 빌드는 포함하지 않으므로 빌드 확인에는 위의 `build` 스크립트를 사용합니다. Rust 정적 분석은 별도로 실행할 수 있습니다.

```bash
cd src/backend
cargo clippy --locked --all-targets -- -D warnings
```

백엔드는 메모리 SQLite로 CRUD·직렬화·OpenAPI·Swagger UI를 검사합니다. 프런트엔드는 jsdom과 모의 `fetch`로 UI 동작을 검사하며 실제 백엔드에 연결하지 않습니다.

## Docker 실행

프로젝트 루트에서 다음 명령을 실행합니다. Bash에서는 `./scripts/container.ps1` 대신 `bash scripts/container.sh`를 사용합니다.

```powershell
./scripts/container.ps1 build
./scripts/container.ps1 up
./scripts/container.ps1 status
./scripts/container.ps1 logs
./scripts/container.ps1 down
```

- `build`: 프런트엔드 빌드 후 Rust 릴리스 이미지 빌드.
- `up` / `start`: 프런트엔드 빌드, 외부 네트워크 `docker-link`가 없으면 생성, Compose 기동. 기존 백엔드 이미지를 강제로 다시 빌드하지는 않습니다.
- `rebuild`: 컨테이너 제거 → 이미지 빌드 → 기동. 백엔드 변경 반영에 사용합니다.
- `restart`: 컨테이너 제거 후 기동. 단순 프로세스 재시작과 다릅니다.
- `down` / `stop`: 컨테이너 중지 및 제거.
- `clean`: 프로젝트 범위를 넘어 Docker 호스트의 중지된 컨테이너, dangling 이미지, 사용하지 않는 볼륨을 정리합니다.

Compose는 `--port 8080`을 전달하고 `8080:8080`을 매핑하므로 <http://localhost:8080>에서 접속합니다. `src/backend/wwwroot`를 `/app/wwwroot`에 읽기 전용으로 바인드합니다. SQLite DB는 컨테이너의 `/app/todos.db`에 있으며 영속 볼륨이 없어 컨테이너 제거 시 데이터가 사라집니다.

Dockerfile은 프런트엔드를 직접 빌드하지 않습니다. 이미지 생성 전에 `wwwroot/`를 준비해야 하며 관리 스크립트가 이 과정을 수행합니다. Dockerfile의 `EXPOSE 8080`과 달리 기본 실행 명령은 포트를 지정하지 않아 단독 실행 시 서버가 `8000`에서 시작합니다.

## Azure 인프라와 배포 상태

[infra/main.bicep](infra/main.bicep)은 구독 범위에서 다음을 구성합니다.

- 리소스 그룹, Log Analytics Workspace
- Basic Container Registry
- Log Analytics에 연결한 Container Apps Environment
- 시스템 할당 관리 ID를 가진 백엔드 Container App과 `AcrPull` 역할

Storage와 AI Foundry 모듈 호출은 주석 처리되어 있습니다. Application Insights와 Action Group도 모듈 파일만 있고 메인 템플릿에서 호출하지 않습니다. 현재 Rust 애플리케이션은 SQLite를 사용하며 Azure SQL이나 AI 서비스에 연결하지 않습니다.

템플릿 컴파일 확인 명령(Azure CLI/Bicep 필요):

```bash
az bicep build --file infra/main.bicep --stdout
```

현재 설정은 TODO 앱의 Azure 배포를 완성하기 전에 다음 사항을 정리해야 합니다.

| 파일 / 설정 | 현재 상태 |
| --- | --- |
| `azure.yaml` | 이름이 `azure-bicep-fsharp-app`으로 남아 있습니다. 백엔드 서비스의 프로젝트는 `./src/backend`이고 Docker 경로는 `./src/docker`, 컨텍스트는 `./src`로 선언되어 있어 실제 Dockerfile 위치와 azd 경로 해석을 확인해야 합니다. |
| `infra/main.parameters.json` | `AZURE_ENV_NAME`, `AZURE_LOCATION`, 소유자 `choo`를 사용합니다. `azdServiceName` 값도 전달하지만 메인 템플릿에는 해당 매개변수가 없습니다. |
| Container App 이미지 | 초기화·업데이트 모듈 모두 `mcr.microsoft.com/k8se/quickstart:latest`를 사용합니다. 빌드한 TODO 이미지로 교체하는 단계가 필요합니다. |
| 포트 | 초기 ingress는 `80`, 업데이트 ingress는 `8080`입니다. Rust 기본 포트는 `8000`이므로 실제 이미지의 실행 인자와 ingress를 일치시켜야 합니다. |
| 프런트엔드 | `azure.yaml`에 프런트엔드 빌드 hook이 없습니다. 이미지 빌드 전에 정적 산출물을 준비해야 합니다. |
| 데이터 | 영속 저장소 연결이 없으며 최대 replica 수는 `10`입니다. 로컬 SQLite 파일을 여러 replica가 공유하는 구성은 아닙니다. |

따라서 현재 파일만으로 `azd up`이 TODO 앱 배포까지 완료한다고 가정하지 마세요. 위 설정을 맞춘 뒤 별도로 배포 검증이 필요합니다. 템플릿 출력은 `RESOURCE_GROUP_ID`, `AZURE_CONTAINER_REGISTRY_ENDPOINT`, `APP_BACKEND_ENDPOINT`이며, 마지막 값은 URL 스킴을 포함하지 않는 FQDN입니다.
