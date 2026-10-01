# FEAT-001: 작업 제목 검증과 생성 폼

- Version: 1
- Status: READY (설계 예시; 실제 구현/검증 완료를 뜻하지 않음)
- Spec owner: example-spec
- Base revision: 프로젝트 적용 시 기록
- Request: 빈 작업 생성 방지와 일관된 오류 안내

## 목표와 범위

현재 동작: 제목 없는 작업이 저장되어 목록에서 구분되지 않는다.
목표: 제목을 정규화하고 같은 규칙을 API와 SolidJS 폼에서 안내한다.
포함: 제목 검증, 생성 API, 폼 오류/저장 완료 표시, 회귀 테스트.
제외: 인증/권한 구현, 제목 중복 검사, 목록 검색, 재시도/오프라인 저장.

## 계약과 설계 제약

- `POST /api/tasks`, JSON `{ "title": string }`.
- 제목 양 끝의 ASCII whitespace(U+0009~000D, U+0020)를 제거한다.
  내부 공백과 비ASCII 공백은 보존한다. Unicode scalar value 수가 1~120이어야 한다.
- Rust `chars().count()`, TypeScript `Array.from(title).length` 기준으로 맞춘다.
  UTF-16 code unit이나 grapheme 개수를 사용하지 않는다.
- 정상: 201 `{ "id": "<opaque-id>", "title": "<normalized-title>" }`, 1개 저장.
- 누락/비문자열/빈 제목/121자 이상: 400
  `{ "error": { "code": "INVALID_TITLE", "message": "제목은 1~120자여야 합니다." } }`.
- 잘못된 JSON: 400 `{ "error": { "code": "INVALID_JSON" } }`, 저장 없음.
- 폼은 label이 있는 입력/추가 버튼을 제공한다. 오류는 `role="alert"` 영역에 표시한다.
  invalid 입력에서는 API를 호출하지 않는다. API 400 오류도 같은 영역에 표시한다.
- 전송 중 버튼 비활성화. 성공 후 입력 비우기/생성 결과 목록 1건 추가.
  500/네트워크 오류 시 입력 유지, “저장하지 못했습니다. 다시 시도해 주세요.” 표시,
  버튼 복구. 새 API는 additive, 기존 API/스키마 변경 없음.
- 사용자 입력은 텍스트로 렌더링하며 HTML로 삽입하지 않는다.

## Acceptance criteria

| AC-ID | Given / When | Then | 검증 |
|---|---|---|---|
| AC-01 | `"  Buy milk\t"`를 정규화 | `"Buy milk"`, 내부 공백 보존 | unit: normalize_title |
| AC-02 | `""`, ASCII 공백뿐, 121 scalar 입력 | INVALID_TITLE | unit + integration: invalid_title |
| AC-03 | 1자, 120자, emoji 120개/121개 | 1/120 허용, 121 거부 | Rust/TS unit: scalar_boundaries |
| AC-04 | API에 `"  Buy milk  "` 전송 | 201, 정규화 제목, DB 1건 | integration: create_task |
| AC-05 | 제목 누락/숫자/invalid 제목/invalid JSON | 계약별 400, DB 변화 없음 | integration: rejected_requests |
| AC-06 | 폼에서 invalid 제출 | alert 표시, POST 호출 0회 | e2e: invalid_form |
| AC-07 | 정상 제출/서버 400/500/네트워크 오류 | 계약대로 입력/목록/버튼/오류 변화 | e2e: submit_states |
| AC-08 | HTML 모양 제목 저장/표시 | 텍스트로 보임, 스크립트 실행 없음 | e2e: literal_title |

## 작업/병렬화 계획

아래 경로는 예시이며 실제 저장소에 맞추어 spec owner가 확정한다.

| 작업 | Owner | 경로 | 의존성 |
|---|---|---|---|
| Rust validation/API | code-backend | backend/src/title.rs, backend/src/tasks.rs | 계약 READY |
| SolidJS 폼 | code-frontend | frontend/src/features/tasks/TaskForm.tsx, title.ts | API 계약 READY |
| 테스트 | test | backend/tests/tasks.rs, frontend/tests/unit/, frontend/tests/integration/, frontend/e2e/ | 인터페이스 확정 |
| 공통 설정/lockfile | code-backend | Cargo.toml, Cargo.lock, frontend/package.json, pnpm-lock.yaml, .agents/verification.env | orchestrator 배정 |
| 리뷰 | reviewer | .artifacts/<run>/review.md | 통합 SHA tool/AC 검증 |

같은 파일 동시 수정 금지. 프론트/백은 계약 확정 후 병렬화 가능.

## 검증 환경

- Rust toolchain, Node/pnpm 버전은 저장소 lock/toolchain 파일로 고정한다.
- 단위: Rust library tests, TS validator tests. 통합: 실제 API와 격리 DB.
- E2E: 실제 Rust 서버와 SolidJS 앱, Playwright. `webServer` 또는 fixture가
  readiness/종료를 관리하고 실행마다 별도 DB와 포트를 쓴다.
- `.agents/verification.env`의 주석 명령을 실제 경로에 맞추어 설정한다.
- 각 runner summary에서 실행 테스트 1개 이상/전부 skip 아님을 확인한다.
- 새 revision에서 verify 및 AC 증거, 독립 리뷰를 작성한다.

## 질문/가정/rollback

필수 질문 없음. 예시에서는 테스트용 DB와 HTTP API가 기존에 있다고 가정한다.
존재하지 않으면 실제 적용 spec에서 준비 작업과 범위를 먼저 정의한다.
위험: 프론트/백 문자 수 차이. 동일 scalar 테스트 벡터로 검증한다.
Rollback: 추가된 경로/컴포넌트를 되돌린다. 스키마 migration 없음.
완료 조건은 루트 gate와 동일하다. 본 문서는 설계 예시이며 PASS 증거가 아니다.
