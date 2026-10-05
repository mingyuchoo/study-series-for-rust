# SolidJS TODO 프런트엔드

SolidJS와 TypeScript로 구현한 TODO 관리 UI입니다. Rust 백엔드의 `/api/todos`에 요청하며, Vite 빌드 결과를 `../backend/wwwroot/`에 생성해 백엔드와 함께 제공합니다.

## 구현된 기능

- TODO 목록 조회, 제목·선택적 설명으로 추가
- 제목·설명 편집과 취소, 완료·미완료 전환, 삭제
- 목록 새로고침, 로딩·빈 목록 표시, 로컬 시간 형식으로 생성 시각 표시
- 초기 조회 요청을 컴포넌트 해제 시 `AbortController`로 취소
- 상단 토글 버튼으로 한국어/영어 전환: 화면 문구, 알림, 접근성 레이블, 날짜 형식에 적용
- 상단 토글 버튼으로 시스템/라이트/다크 테마 선택: 시스템 모드는 OS 테마 변경을 실시간 반영
- 언어·테마 선택을 `localStorage`의 `todo.language`, `todo.theme`에 저장하고 재방문 시 복원

언어의 초기값은 브라우저 언어가 한국어이면 한국어, 그 외에는 영어입니다. 테마의 초기값은 시스템입니다. 저장소 사용이 차단되어도 현재 화면에서 전환할 수 있습니다. 언어나 테마를 바꿔도 작성·편집 중인 내용과 검색·필터 상태는 유지됩니다. 번역 문구는 `src/i18n.ts`, 설정 상태와 브라우저 연동은 `src/preferences.ts`에서 관리합니다.

[src/App.tsx](src/App.tsx)에서 `createSignal`로 상태를 관리하고 `<For>`로 목록을 렌더링합니다. 생성·수정·삭제 성공 후 목록을 다시 조회합니다. 추가 시 공백만 있는 제목은 무시하지만 입력값 자체를 잘라서 저장하지는 않습니다.

네트워크 예외는 브라우저 콘솔에 기록하며 사용자용 오류 메시지나 재시도 UI는 없습니다. 초기 요청 취소는 최초 조회에 적용되고, 이후 모든 요청을 일괄 취소하는 구현은 아닙니다.

## 개발 환경과 도구

아래 값은 [package.json](package.json)에 선언된 버전 요구사항입니다. 실제 설치 버전은 [pnpm-lock.yaml](pnpm-lock.yaml)을 따릅니다.

| 항목 | 설정 |
| --- | --- |
| Node.js | `^20.19.0 \|\| ^22.13.0 \|\| >=24`; [../.node-version](../.node-version)은 `v24.2.0` |
| 패키지 관리자 | `pnpm@12.9.1` |
| UI | `solid-js` `^1.9.15` |
| 빌드 | `vite` `^8.3.2`, `vite-plugin-solid` `^2.11.14` |
| 타입 검사 | `@typescript/native`: `npm:typescript@^7.0.2` |
| TypeScript 호환 패키지 | `typescript`: `npm:@typescript/typescript6@^6.0.2` |
| 린트 | ESLint `^10.12.0`, typescript-eslint `^8.71.0`, eslint-plugin-solid `^0.18.1` |
| 테스트 | Vitest `^5.0.3`, jsdom `^30.1.2` |

TypeScript는 두 별칭으로 설치됩니다. 빌드의 `tsc -b`에는 TypeScript 7 컴파일러를 사용하고, `typescript` 모듈에는 ESLint 도구 호환용 TypeScript 6 패키지를 제공합니다. 별칭과 잠금 파일을 함께 유지하세요.

[pnpm-workspace.yaml](pnpm-workspace.yaml)은 `esbuild` 빌드 스크립트를 허용하지 않으며, 일부 jsdom 관련 패키지에 `minimumReleaseAgeExclude`를 지정합니다.

## 개발 서버 실행

터미널 1에서 백엔드를 시작합니다(프로젝트 루트 기준).

```bash
cd src/backend
cargo run --locked
```

터미널 2에서 프런트엔드를 실행합니다(프로젝트 루트 기준).

```bash
cd src/frontend
npx --yes pnpm@12.9.1 install --frozen-lockfile
npx --yes pnpm@12.9.1 run dev
```

기본 접속 주소는 <http://localhost:5173>입니다. 포트 사용 여부에 따라 달라질 수 있으므로 Vite의 터미널 출력을 확인하세요. 이미 지정된 버전의 pnpm을 사용하고 있다면 `npx --yes pnpm@12.9.1` 대신 `pnpm`으로 실행해도 됩니다.

[vite.config.ts](vite.config.ts)의 `server.proxy`는 `/api`로 시작하는 요청을 `http://localhost:8000`으로 전달합니다. 백엔드 포트를 바꾸면 이 설정도 변경해야 합니다. Swagger UI는 <http://localhost:8000/swagger-ui/>에서 직접 확인합니다.

## npm 스크립트

다음 명령은 `src/frontend`에서 실행합니다.

| 명령 | 동작 |
| --- | --- |
| `pnpm run dev` | Vite 개발 서버 |
| `pnpm run build` | `tsc -b` 후 Vite 프로덕션 빌드 |
| `pnpm run build:backend` | 같은 타입 검사와 빌드, CLI로 백엔드 출력 경로 명시 |
| `pnpm run preview` | 이미 생성한 빌드 결과를 Vite preview로 제공 |
| `pnpm run lint` | `eslint .` |
| `pnpm test` | `vitest run`으로 실행 후 종료 |

### 통합 빌드와 실행

```bash
npx --yes pnpm@12.9.1 run build
cd ../backend
cargo run --locked
```

<http://localhost:8000>에서 빌드한 UI와 API를 함께 사용합니다. `build`와 `build:backend` 모두 `../backend/wwwroot/`에 출력하며, `emptyOutDir: true` 때문에 빌드할 때 해당 디렉터리를 비웁니다. 수동으로 보관할 파일을 이 디렉터리에 넣지 마세요.

`preview`는 프런트엔드 빌드 결과를 확인하는 명령이며 Rust 서버를 기동하지 않습니다. API가 필요한 TODO 동작을 확인할 때는 백엔드도 별도로 실행해야 합니다. 통합 실행은 위의 백엔드 주소로 확인할 수 있습니다.

## API 연결

모든 요청은 상대 경로를 사용하며 API 기본 URL 환경변수는 현재 없습니다. 개발 시 Vite 프록시를 거치고, 백엔드가 UI를 제공하는 통합 실행에서는 동일한 출처의 API에 직접 연결합니다.

| 동작 | 요청 | 본문 |
| --- | --- | --- |
| 목록 / 새로고침 | `GET /api/todos` | 없음 |
| 추가 | `POST /api/todos` | `title`, `description` |
| 편집 저장 | `PUT /api/todos/{id}` | `title`, `description` |
| 완료 전환 | `PUT /api/todos/{id}` | `completed` |
| 삭제 | `DELETE /api/todos/{id}` | 없음 |

생성·수정 요청은 `Content-Type: application/json`을 사용합니다. 서버의 TODO 응답에는 `id`, `title`, `description`(문자열 또는 `null`), `completed`, `created_at`, `updated_at`이 포함됩니다. 자세한 계약은 [백엔드 README](../backend/README.md#api)를 참고하세요.

## 테스트와 검증

```bash
npx --yes pnpm@12.9.1 run lint
npx --yes pnpm@12.9.1 test
npx --yes pnpm@12.9.1 run build
```

[src/App.test.tsx](src/App.test.tsx)는 Vitest + jsdom 환경에서 Solid 컴포넌트를 렌더링하고 `fetch`를 모의 구현합니다.

- 조회·추가·편집·취소·완료 전환·새로고침·삭제 흐름
- 컴포넌트 해제 시 초기 조회 요청 취소
- 초기 네트워크 요청 실패 후 로딩 표시 종료

이 테스트는 실제 HTTP 서버나 브라우저를 사용하는 E2E 테스트가 아닙니다. 타입 검사와 프로덕션 번들 검증은 `build` 명령으로 별도 수행합니다.

## 코드 구조

```text
src/frontend/
├── src/
│   ├── App.tsx             # TODO UI와 API 요청
│   ├── App.test.tsx        # 컴포넌트 동작 테스트
│   ├── App.css             # 앱 스타일
│   ├── index.css           # 전역 스타일
│   ├── main.tsx            # Solid render 진입점
│   └── vite-env.d.ts       # Vite 타입 참조
├── public/vite.svg         # 정적 자산
├── index.html              # HTML 진입점
├── package.json            # 실행 명령과 의존성
├── pnpm-lock.yaml          # 의존성 잠금 파일
├── pnpm-workspace.yaml     # pnpm 설치 정책
├── vite.config.ts          # Solid 플러그인, 빌드 출력, 개발 프록시
├── vitest.config.ts        # jsdom, 테스트용 Solid 플러그인
├── eslint.config.js        # TypeScript / Solid 린트
└── tsconfig*.json          # 앱·도구용 TypeScript 프로젝트 설정
```

Docker 관리 스크립트는 프런트엔드를 먼저 빌드합니다. Dockerfile 자체에는 Node.js 빌드 단계가 없으므로 이미지에 UI를 포함하려면 산출물이 미리 준비되어 있어야 합니다. Docker와 Azure 배포 설정은 [루트 README](../../README.md)를 참고하세요.
