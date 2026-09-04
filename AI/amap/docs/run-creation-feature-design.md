# 현대화 작업 생성 기능 설계

## 1. 문서 목적

AMAP 웹 운영 콘솔에서 사용자가 다음 정보를 명시한 뒤 현대화 실행을 시작할 수 있도록 기능을 설계한다.

1. 작업 이름
2. 현대화할 원본 소스 위치
3. 현대화 결과가 생성될 위치
4. 실행 명세와 모의 실행 여부

이 문서는 화면, API, 데이터 모델, 검증, 보안, 오류 처리와 인수 조건을 구현 가능한 수준으로 정의한다.

## 2. 현재 상태와 문제

현재 웹의 `NewRunPanel`은 다음 값만 `POST /v1/runs`에 전달한다.

```json
{
  "spec": "examples/loan-demo/amap.toml",
  "mock": false
}
```

원본 소스와 출력 작업공간은 선택한 `amap.toml`의 `[run]`에 고정되어 있다.

```toml
[run]
source_root = "legacy"
workspace = ".workspace/next"
```

이 구조에는 다음 문제가 있다.

- 실제 실행 대상을 사용자가 화면에서 확인하거나 바꿀 수 없다.
- 결과가 어느 디렉터리에 생성되는지 실행 전에 알기 어렵다.
- 최근 실행 목록이 업무 기능 ID만 보여 주므로 실행 목적을 구분하기 어렵다.
- 잘못된 대상이나 기존 결과 디렉터리를 사용해도 실행 버튼을 누르기 전에는 알 수 없다.
- 운영자가 승인한 입력과 실제 실행에 사용된 입력을 사후 감사하기 어렵다.

## 3. 설계 결정

### 3.1 실행 명세는 템플릿으로 사용한다

`amap.toml`은 명령, 비교 규칙, 불변식, 품질 임계값 등 전문가가 관리하는 실행 템플릿이다. 웹에서 입력한 원본 및 결과 위치는 명세 파일 자체를 수정하지 않고, 해당 실행을 위해 메모리에서 복제한 `RunSpec`에만 적용한다.

우선순위는 다음과 같다.

```text
웹/API 실행 입력 > amap.toml 기본값
```

명세의 기존 `source_root`와 `workspace`는 화면의 초기값으로 사용한다. 따라서 CLI와 기존 API 사용자는 기존 방식으로 계속 실행할 수 있다.

### 3.2 사용자 용어는 “작업”, 내부 용어는 `WorkflowRun`을 유지한다

이번 범위에서는 작업 하나가 실행 하나에 대응한다. 사용자가 입력하는 `작업 이름`을 `WorkflowRun.display_name`에 저장한다. 재사용 가능한 프로젝트와 여러 차수의 실행을 묶는 별도 `ModernizationJob` 엔터티는 후속 범위로 둔다.

### 3.3 브라우저가 서버 파일시스템을 직접 탐색하지 않는다

웹 브라우저의 파일 선택기는 브라우저가 실행 중인 PC의 파일을 선택하며, Control Plane 호스트의 경로를 안전하게 선택할 수 없다. 따라서 경로 선택 UI는 Control Plane이 허용된 `worker_root` 하위 디렉터리만 반환하는 전용 API를 사용한다.

API와 화면에는 호스트 절대 경로 대신 `worker_root` 기준 상대 경로만 노출한다.

### 3.4 실행 전 점검과 실행 시 검증을 모두 수행한다

`사전 점검`은 사용자 경험을 위한 것이며 보안 경계가 아니다. `POST /v1/runs`는 경로, 권한, 충돌 여부를 다시 검증하고 검증에 성공한 값만 실행해야 한다.

## 4. 범위

### 4.1 포함

- 작업 이름 입력과 자동 제안
- 서버 디렉터리 탐색 및 직접 경로 입력
- 원본 소스 및 결과 위치의 실행별 재정의
- 실행 전 사전 점검
- 경로 충돌과 보안 경계 검증
- 실행 입력 스냅샷 저장 및 상세 화면 표시
- 과거 실행 데이터와 API의 하위 호환

### 4.2 제외

- Git 저장소 clone, 원격 브랜치 및 Pull Request 생성
- 소스 파일 업로드
- 결과 디렉터리의 기존 파일 삭제 또는 자동 덮어쓰기
- 여러 실행을 하나의 장기 작업으로 묶는 프로젝트 관리
- 운영체제의 임의 경로 탐색

Git URL, ref, 출력 저장소 및 PR 생성은 `source.type`과 `destination.type`을 확장하는 후속 단계로 설계한다.

## 5. 사용자 흐름

```text
새 작업 열기
    ↓
실행 명세 선택
    ↓ 명세 기본값 로드
작업 이름·원본·결과 위치 입력
    ↓ 입력 변경 시 기존 점검 결과 무효화
사전 점검
    ├─ 실패 → 필드별 오류 수정
    └─ 성공 → 실행 요약 확인
                  ↓
              실행 시작
                  ↓ 서버에서 동일 검증 재수행
            ├─ 충돌 → 실행하지 않고 오류 표시
            └─ 성공 → 작업 기록 저장 및 파이프라인 시작
```

## 6. 화면 설계

### 6.1 왼쪽 실행 패널

현재 패널 폭으로는 경로 입력과 오류 메시지를 표현하기 어렵다. `새 워크플로`를 누르면 중앙 영역에 `새 현대화 작업` 폼을 열고, 실행이 시작되면 기존 모니터링 화면으로 전환한다.

```text
┌─ 새 현대화 작업 ──────────────────────────────────────────┐
│ 작업 정보                                                   │
│ 작업 이름 *    [대출 조기상환 현대화 - 2026-09-05         ]│
│ 실행 명세 *    [Loan early repayment posting            ▾]│
│               P0 · Loan · FN-LOAN-0001                     │
│                                                            │
│ 원본 소스                                                   │
│ 소스 경로 *    [examples/loan-demo/legacy             ][선택]│
│               ✓ 디렉터리 · 2개 소스 파일                   │
│                                                            │
│ 결과 위치                                                   │
│ 결과 경로 *    [examples/loan-demo/.workspace/next    ][선택]│
│               새 디렉터리를 생성합니다.                    │
│                                                            │
│ 실행 옵션                                                   │
│ □ fixture 기반 모의 LLM 사용                               │
│                                                            │
│ [사전 점검]                         [취소] [실행 시작]      │
└────────────────────────────────────────────────────────────┘
```

### 6.2 필드 정의

| 필드 | 필수 | 초기값 | 규칙 |
|---|---:|---|---|
| 작업 이름 | 예 | `<명세 이름> - <현재 날짜>` | trim 후 1~80자, 제어문자 금지, 중복 허용 |
| 실행 명세 | 예 | 첫 번째 사용 가능 명세 | 기존 `SpecSummary` 목록 사용 |
| 소스 경로 | 예 | 명세의 `source_root` | `worker_root` 기준 상대 경로, 존재하는 읽기 가능한 디렉터리 |
| 결과 경로 | 예 | 명세의 `workspace` | `worker_root` 기준 상대 경로, 생성 가능한 위치 |
| 모의 LLM | 아니요 | false | 명세에 fixture가 있고 insecure 개발 모드일 때만 활성화 |

`작업 이름`은 업무 기능의 이름이나 ID와 별개이다. 예를 들어 업무 기능은 `Loan early repayment posting`, 작업 이름은 `대출 조기상환 Rust 전환 1차`가 될 수 있다.

### 6.3 디렉터리 선택 대화상자

- 현재 상대 경로와 상위 경로 이동을 제공한다.
- 디렉터리만 표시하며 일반 파일과 심볼릭 링크는 선택할 수 없다.
- `worker_root` 위로 이동할 수 없다.
- 숨김 디렉터리는 기본적으로 숨긴다. 결과 경로의 명세 기본값처럼 필요한 숨김 경로는 직접 입력할 수 있다.
- 원본 선택은 기존 디렉터리만 확정할 수 있다.
- 결과 선택은 기존 부모 디렉터리를 선택하고 새 하위 디렉터리명을 입력할 수 있다.
- 경로가 길면 말줄임표 대신 줄바꿈하고 전체 상대 경로를 유지한다.

### 6.4 버튼 상태

| 상태 | 사전 점검 | 실행 시작 |
|---|---:|---:|
| 필수값 누락 | 비활성 | 비활성 |
| 입력 완료, 미점검 | 활성 | 비활성 |
| 점검 중 | 로딩 | 비활성 |
| 점검 실패 | 활성 | 비활성 |
| 점검 성공 | 활성 | 활성 |
| 점검 후 입력 변경 | 활성 | 비활성 |
| 실행 요청 중 | 비활성 | 로딩/비활성 |

### 6.5 실행 후 표시

최근 실행 목록의 기본 행은 다음 순서로 표시한다.

```text
대출 조기상환 Rust 전환 1차      [실행 중]
FN-LOAN-0001 · 9월 5일 02:40
```

실행 상세 상단에는 다음 값을 표시한다.

- 작업 이름
- 내부 실행 ID
- 업무 기능 ID 및 명세 이름
- 원본 소스 상대 경로
- 결과 상대 경로
- 실행을 시작한 사용자와 시간

실제 호스트 절대 경로는 일반 운영자 화면과 API 응답에 노출하지 않는다.

## 7. API 설계

### 7.1 명세 목록 확장

`GET /v1/specs`

`SpecSummary`에 화면 초기값을 추가한다.

```json
{
  "path": "examples/loan-demo/amap.toml",
  "function_id": "FN-LOAN-0001",
  "name": "Loan early repayment posting",
  "domain": "Loan",
  "priority": "P0",
  "description": "...",
  "mock_available": true,
  "defaults": {
    "source_path": "examples/loan-demo/legacy",
    "destination_path": "examples/loan-demo/.workspace/next"
  }
}
```

명세의 경로가 `worker_root` 밖을 가리키면 기본값을 반환하지 않고 명세를 실행 불가 상태로 표시한다.

```json
{
  "runnable": false,
  "unavailable_reason": "source_path_outside_worker_root"
}
```

### 7.2 안전한 디렉터리 탐색

`GET /v1/paths?purpose=source&parent=examples/loan-demo`

`purpose`는 `source` 또는 `destination`이다.

```json
{
  "path": "examples/loan-demo",
  "parent": "examples",
  "entries": [
    {
      "name": "legacy",
      "path": "examples/loan-demo/legacy",
      "kind": "directory",
      "readable": true,
      "writable": true
    }
  ]
}
```

응답은 디렉터리 이름 기준 오름차순이며 최대 200개를 반환한다. 제한을 넘으면 검색 문자열을 요구한다. 심볼릭 링크와 `worker_root` 밖으로 해석되는 항목은 반환하지 않는다.

### 7.3 사전 점검

`POST /v1/runs/preflight`

```json
{
  "name": "대출 조기상환 Rust 전환 1차",
  "spec": "examples/loan-demo/amap.toml",
  "source": {
    "type": "local_path",
    "path": "examples/loan-demo/legacy"
  },
  "destination": {
    "type": "local_path",
    "path": "examples/loan-demo/.workspace/next"
  },
  "mock": false
}
```

성공 응답:

```json
{
  "valid": true,
  "validation_token": "short-lived-signed-token",
  "expires_at": "2026-09-05T03:05:00Z",
  "effective": {
    "name": "대출 조기상환 Rust 전환 1차",
    "function_id": "FN-LOAN-0001",
    "spec": "examples/loan-demo/amap.toml",
    "source_path": "examples/loan-demo/legacy",
    "destination_path": "examples/loan-demo/.workspace/next",
    "source_file_count": 2,
    "destination_state": "will_create"
  },
  "errors": [],
  "warnings": []
}
```

실패 응답도 HTTP 200으로 반환하여 여러 필드 오류를 한 번에 보여 준다.

```json
{
  "valid": false,
  "errors": [
    {
      "field": "destination.path",
      "code": "path_overlaps_source",
      "message": "결과 위치는 원본 소스와 겹칠 수 없습니다."
    }
  ],
  "warnings": []
}
```

인증 실패, 요청 크기 초과와 같은 요청 자체의 실패는 기존 HTTP 오류 상태를 사용한다.

`validation_token`은 점검한 입력의 해시, 사용자, 만료 시간을 서명한 값이다. 실행 API는 토큰만 신뢰하지 않고 전체 검증을 반복한다. 토큰은 화면의 점검 상태와 감사 로그를 연결하기 위해 사용한다.

### 7.4 실행 시작 확장

`POST /v1/runs`

```json
{
  "name": "대출 조기상환 Rust 전환 1차",
  "spec": "examples/loan-demo/amap.toml",
  "source": {
    "type": "local_path",
    "path": "examples/loan-demo/legacy"
  },
  "destination": {
    "type": "local_path",
    "path": "examples/loan-demo/.workspace/next"
  },
  "mock": false,
  "validation_token": "short-lived-signed-token"
}
```

하위 호환을 위해 `name`, `source`, `destination`, `validation_token`은 API 스키마에서 한 릴리스 동안 선택값으로 둔다. 생략하면 다음 값을 사용한다.

- `name`: `<function.name> - <started_at>`
- `source`: 명세의 `source_root`
- `destination`: 명세의 `workspace`

웹 UI는 네 값을 모두 전달해야 한다. 다음 메이저 API에서 `name`, `source`, `destination`을 필수로 전환한다.

주요 오류 코드는 다음과 같다.

| HTTP | 코드 | 의미 |
|---:|---|---|
| 400 | `invalid_name` | 이름 형식 오류 |
| 400 | `source_not_found` | 소스 디렉터리가 없음 |
| 400 | `destination_not_creatable` | 결과 위치 생성 불가 |
| 409 | `destination_conflict` | 점검 이후 결과 위치가 점유됨 |
| 409 | `path_overlap` | 원본과 결과 경로가 겹침 |
| 403 | `path_outside_worker_root` | 허용 경계 밖 경로 |
| 403 | `mock_not_allowed` | 운영 환경의 모의 실행 요청 |

오류 본문은 문자열 대신 일관된 JSON Problem 형식으로 변경한다.

```json
{
  "code": "destination_conflict",
  "message": "결과 위치가 비어 있지 않습니다.",
  "field": "destination.path",
  "request_id": "REQ-..."
}
```

## 8. 데이터 모델

기존 `WorkflowRun`을 다음과 같이 확장한다.

```rust
pub struct WorkflowRun {
    pub id: String,
    pub display_name: String,
    pub function_id: FunctionId,
    pub status: String,
    pub spec_path: PathBuf,
    pub inputs: RunInputs,
    pub mock: bool,
    pub started_by: String,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub outcome: Option<Value>,
    pub checkpoint: Value,
}

pub struct RunInputs {
    pub source: LocalPathInput,
    pub destination: LocalPathInput,
}

pub struct LocalPathInput {
    pub kind: InputKind, // local_path
    pub path: PathBuf,   // worker_root 기준 상대 경로
}
```

실행에 사용하는 절대 경로는 요청 처리 중에만 `RunSpec.run.source_root`와 `RunSpec.run.workspace`에 적용한다. 저장 데이터와 API 응답에는 상대 경로를 기록한다.

Control Plane은 저장 모델인 `WorkflowRun`을 API에 그대로 직렬화하지 않고 별도의 `WorkflowRunResponse`로 변환한다. 현재 절대 경로로 저장되는 `spec_path`는 기존 재개 로직과의 호환을 위해 내부 저장값으로 유지할 수 있지만, 응답에는 `spec_root` 기준의 `spec` 상대 경로만 반환한다. 신규 입력인 source와 destination도 응답에서 항상 `worker_root` 기준 상대 경로이다.

PostgreSQL은 `workflow_runs.doc` JSONB에 전체 구조를 저장하므로 컬럼 마이그레이션은 필요 없다. Rust 역직렬화에는 기존 데이터용 기본값을 둔다.

- `display_name` 누락: `function_id` 사용
- `inputs` 누락: 저장된 `spec_path`의 명세에서 값을 읽어 표시하되 `legacy_inferred: true` 표시
- `started_by` 누락: `unknown`

목록 검색이 추가될 때 다음 표현식 인덱스를 별도 마이그레이션으로 고려한다.

```sql
CREATE INDEX idx_workflow_runs_display_name
ON workflow_runs ((doc->>'display_name'));
```

## 9. 서버 처리 순서

`POST /v1/runs`는 다음 순서를 지킨다.

1. 사용자를 인증하고 요청 ID를 만든다.
2. 작업 이름을 trim하고 형식을 검증한다.
3. 명세 경로를 `spec_root` 안에서 canonicalize한다.
4. `RunSpec`을 로드한다.
5. 요청 경로를 `worker_root` 기준으로 안전하게 해석한다.
6. 원본이 존재하는 실제 디렉터리인지 확인한다.
7. 결과 경로의 가장 가까운 기존 부모를 canonicalize한다.
8. 원본과 결과가 같거나 상호 포함 관계인지 확인한다.
9. 결과 위치가 없거나 비어 있는지 확인한다.
10. 복제한 `RunSpec`의 `source_root`와 `workspace`를 검증된 절대 경로로 교체한다.
11. 기존 `validate_execution_boundary`로 지원 파일, 명령 및 실행 파일 허용 목록을 검증한다.
12. 결과 디렉터리를 원자적으로 예약하고 소유권 메타데이터를 기록한다.
13. 사용자 입력 스냅샷을 포함한 `WorkflowRun`을 저장한다.
14. 감사 이벤트를 기록한 뒤 파이프라인을 시작한다.

### 9.1 중단 실행 재개

`POST /v1/runs/{id}/resume`은 명세만 다시 읽어서는 안 된다. 저장된 `RunInputs`를 다시 검증하고 복제한 `RunSpec`에 동일하게 적용한 뒤 `context_for`를 호출한다. 그래야 최초 실행과 재개 실행이 같은 원본 및 결과 위치를 사용한다.

재개 시 원본이 사라졌거나 결과 위치의 소유권 메타데이터가 다른 run으로 바뀌었다면 실행하지 않고 `409 run_inputs_changed`를 반환한다. 기존 레코드처럼 `RunInputs`가 없을 때만 명세의 기본값을 사용한다.

결과 디렉터리의 `.amap/run.json` 예약 파일 예시:

```json
{
  "run_id": "RUN-...",
  "created_by": "operator-sub",
  "created_at": "2026-09-05T03:00:00Z"
}
```

예약 이후 실행 시작에 실패하면 디렉터리가 비어 있고 예약 파일만 있을 때에 한해 서버가 정리할 수 있다. 사용자 파일이 하나라도 있으면 자동 삭제하지 않는다.

## 10. 검증 규칙

### 10.1 이름

- Unicode를 허용한다.
- 앞뒤 공백을 제거한다.
- 빈 문자열, NUL 및 제어문자를 거부한다.
- 최대 80자는 Unicode 문자 수 기준으로 검사한다.
- HTML로 렌더링할 때 문자열 삽입만 사용하고 HTML을 직접 주입하지 않는다.
- 이름은 식별자가 아니므로 중복을 허용한다. 내부 식별에는 기존 `run_id`를 사용한다.

### 10.2 원본 경로

- 절대 경로와 `..` 구성 요소를 거부한다.
- `worker_root` 기준으로 해석한다.
- 대상이 존재하고 디렉터리여야 한다.
- 심볼릭 링크 자체 및 심볼릭 링크를 통한 경계 이탈을 거부한다.
- Control Plane 프로세스가 읽을 수 있어야 한다.
- 기본 소스 파일 확장자가 하나도 없으면 경고하되 실행은 허용한다.

기본 탐색 확장자는 현재 discovery 지원 범위인 `.cbl`, `.cob`, `.cpy`, `.rs`, `.js`, `.jsx`, `.ts`, `.tsx`, `.cs`, `.sql`, `.jcl`을 사용한다.

### 10.3 결과 경로

- 절대 경로와 `..` 구성 요소를 거부한다.
- `worker_root` 기준으로 해석한다.
- 가장 가까운 기존 부모가 실제 디렉터리이고 쓰기 가능해야 한다.
- 원본과 동일하거나 원본의 자식 또는 부모이면 거부한다.
- 기존에 존재하며 비어 있지 않으면 거부한다.
- 심볼릭 링크 및 심볼릭 링크를 통한 경계 이탈을 거부한다.
- 이번 범위에서는 기존 파일 덮어쓰기 옵션을 제공하지 않는다.

### 10.4 명세와 실행 명령

명세의 `next_command`와 `legacy_command`는 실행별 경로를 적용한 뒤 다시 해석한다. `{workspace}`와 `{source_root}` 자리표시자를 사용하는 명령은 새 경로를 반영한다.

자리표시자가 없고 기존 명세의 특정 경로를 직접 참조하는 인자는 사용자가 경로를 변경해도 자동 변경되지 않는다. 사전 점검은 이를 경고한다.

```json
{
  "field": "spec",
  "code": "command_may_ignore_override",
  "message": "실행 명령이 {source_root} 또는 {workspace} 자리표시자를 사용하지 않습니다."
}
```

## 11. 보안 및 감사

- 디렉터리 목록, 사전 점검 및 실행 API는 모두 기존 인증 미들웨어 뒤에 둔다.
- `spec_root`와 `worker_root`는 서로 다른 보안 경계로 계속 유지한다.
- API는 호스트의 절대 경로, 소유자 이름, 권한 비트 등 불필요한 정보를 노출하지 않는다.
- 모든 경로는 문자열 prefix 비교가 아니라 canonical path 비교를 사용한다.
- 존재하지 않는 결과 경로는 가장 가까운 기존 부모를 canonicalize한 다음 나머지 구성 요소를 검증한다.
- 경로 탐색에서 심볼릭 링크를 따라가지 않는다.
- 실행 시작 감사 이벤트에는 `run_id`, 사용자, 작업 이름, 명세, 원본 상대 경로, 결과 상대 경로와 mock 여부를 기록한다.
- 민감한 토큰이나 환경 변수는 입력 스냅샷에 저장하지 않는다.
- 실행 상세의 입력 값은 실행 후 수정할 수 없는 스냅샷이다.

## 12. 상태와 오류 UX

- 필드 오류는 해당 입력 바로 아래에 표시하고 첫 오류 필드로 포커스를 이동한다.
- 서버 연결 실패는 폼 상단에 표시하며 사용자의 입력을 유지한다.
- 사전 점검 성공 후 서버 상태가 바뀌어 실행이 충돌하면 `결과 위치가 점검 이후 변경되었습니다`라고 표시하고 점검 상태를 해제한다.
- 실행 요청을 중복 클릭해도 하나만 생성되도록 버튼을 즉시 비활성화하고 요청에 idempotency key를 사용한다.
- 새로 고침 후 미완성 폼 복구는 이번 범위에서 제외한다. API 토큰과 달리 경로와 작업 이름을 브라우저 저장소에 남기지 않는다.

## 13. 접근성

- 모든 필드는 표시되는 `<label>`과 연결한다.
- 필수 여부를 색상에만 의존하지 않고 텍스트와 `required`/`aria-required`로 표현한다.
- 오류는 `aria-describedby`와 연결하고 사전 점검 결과 영역은 `aria-live="polite"`를 사용한다.
- 대화상자는 포커스 트랩, ESC 닫기와 닫은 뒤 원래 버튼으로 포커스 복귀를 지원한다.
- 버튼 및 상태의 의미는 아이콘만으로 표현하지 않는다.

## 14. 구현 영향 범위

| 영역 | 대상 파일 | 변경 요약 |
|---|---|---|
| OpenAPI | `openapi/amap.yaml` | 실행 입력, 사전 점검, 경로 탐색 및 확장된 실행 응답 정의 |
| 타입 생성 | `web/src/api/schema.d.ts` | OpenAPI에서 재생성 |
| 웹 API | `web/src/api/client.ts` | 경로 목록, 사전 점검, 확장된 실행 요청 추가 |
| 웹 화면 | `web/src/App.tsx` | 작업 생성 폼, 점검 상태, 목록/상세의 작업 이름 표시 |
| 웹 스타일 | `web/src/styles.css` | 전체 폭 폼, 경로 선택기, 필드 오류와 요약 스타일 |
| Control Plane | `services/control-plane/src/main.rs` | 요청 모델, 경로 목록, 사전 점검, 실행별 override와 감사 처리 |
| 실행 상태 | `crates/knowledge/src/lib.rs` | `display_name`, `inputs`, `started_by` 추가 |
| 실행 명세 | `crates/platform/src/runspec.rs` | 검증된 실행별 override 적용 보조 함수 추가 |
| 검증 | `crates/orchestrator/src/config.rs` | 경로 중첩, 출력 예약 및 경계 검증 보강 |
| 문서 | `README.md` | 웹 실행 생성 방법 및 보안 경계 설명 갱신 |

## 15. 테스트 설계

### 15.1 서버 단위 테스트

- 한글, 공백 및 최대 길이 작업 이름 허용
- 빈 이름, 제어문자 및 길이 초과 거부
- 정상적인 source/destination 상대 경로 해석
- 절대 경로와 `..` 거부
- source 미존재, 파일인 source, 읽기 불가 source 거부
- destination이 source와 동일/부모/자식인 경우 거부
- destination의 기존 비어 있지 않은 디렉터리 거부
- worker root 밖 심볼릭 링크 거부
- 명세 기본값 사용 시 기존 요청 호환
- runtime override가 원본 `amap.toml`을 수정하지 않음
- 중단 후 재개 시 저장된 source/destination override를 다시 적용
- `{source_root}` 및 `{workspace}` 치환 결과 검증
- 운영 모드의 mock 실행 거부 유지
- 기존 `WorkflowRun` JSON 역직렬화

### 15.2 API 계약 테스트

- OpenAPI 스키마와 실제 성공/오류 응답 일치
- 경로 목록에서 파일, 심볼릭 링크 및 root 밖 항목 제외
- preflight가 여러 필드 오류를 함께 반환
- preflight 후 충돌 발생 시 start가 409 반환
- 동일 idempotency key가 중복 실행을 만들지 않음

### 15.3 웹 테스트

- 명세 선택 시 기본 경로와 제안 이름 설정
- 사용자가 수정한 이름은 명세 변경 시 확인 없이 덮어쓰지 않음
- 필수값 누락 및 미점검 상태에서 실행 버튼 비활성
- 점검 성공 후 입력 변경 시 실행 버튼 다시 비활성
- 필드별 서버 오류 연결
- 성공 시 `/runs/{id}`로 이동
- 최근 목록과 상세에 `display_name` 표시
- 키보드만으로 경로 선택 대화상자 사용 가능

### 15.4 E2E 시나리오

1. 대출 명세 선택
2. 작업 이름 수정
3. `examples/loan-demo/legacy`를 원본으로 선택
4. 존재하지 않는 새 결과 디렉터리 입력
5. 사전 점검 성공 확인
6. 실행 시작
7. 최근 실행과 상세 화면에서 이름 및 두 경로 확인
8. 결과 디렉터리에 해당 run의 예약 메타데이터 확인
9. 동일 결과 위치로 두 번째 실행 시 충돌 오류 확인

## 16. 인수 조건

- 사용자는 실행 전에 작업 이름, 원본 소스 및 결과 위치를 확인하고 변경할 수 있다.
- 사용자는 `worker_root` 안의 디렉터리를 화면에서 선택하거나 상대 경로를 직접 입력할 수 있다.
- 잘못되거나 위험한 경로에서는 실행 버튼이 활성화되지 않는다.
- 서버는 UI 점검 여부와 무관하게 동일한 보안 검증을 수행한다.
- 원본과 결과 경로가 겹치거나 결과 위치가 비어 있지 않으면 실행을 시작하지 않는다.
- 기존 파일을 자동 삭제하거나 덮어쓰지 않는다.
- 실행에 사용된 입력이 `WorkflowRun`에 불변 스냅샷으로 남는다.
- 최근 실행 목록은 기능 ID보다 작업 이름을 우선 표시한다.
- 기존 `{ "spec": "...", "mock": false }` 요청과 과거 실행 레코드를 읽을 수 있다.
- CLI의 기존 run spec 기반 실행 동작은 변경되지 않는다.

## 17. 단계별 구현 순서

### 1단계: 계약과 데이터

- OpenAPI 모델 추가
- `WorkflowRun` 하위 호환 필드 추가
- 경로 해석 및 검증 함수를 독립 모듈로 분리

### 2단계: Control Plane

- 명세 기본값 확장
- 안전한 경로 목록 API
- 사전 점검 API
- 실행별 override, 재검증, 출력 예약 및 감사 이벤트

### 3단계: 웹

- 전체 폭 작업 생성 폼
- 디렉터리 선택 대화상자
- 사전 점검과 필드 오류
- 목록 및 상세의 작업 메타데이터 표시

### 4단계: 검증과 문서

- Rust, API, React 및 E2E 테스트
- OpenAPI 타입 재생성
- README 운영 절차와 보안 설명 갱신

## 18. 후속 확장

MVP의 구조화된 `source`와 `destination` 객체는 다음 타입으로 확장할 수 있다.

```text
source.type:
  local_path | git

destination.type:
  local_path | git_branch
```

Git 입력에는 저장소 URL, credential reference, ref 및 하위 경로를 추가하고, Git 출력에는 저장소, base branch, 작업 branch와 PR 생성 정책을 추가한다. 자격 증명 자체는 실행 요청이나 `WorkflowRun`에 저장하지 않고 비밀 저장소의 참조만 보관한다.
