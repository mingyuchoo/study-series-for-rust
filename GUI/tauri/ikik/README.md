# IKIK

Clean Architecture 기반의 개인 목표와 성과 관리 데스크톱 앱입니다. Rob Moore의 큰 그림에서 세부 실행으로 내려가는 모델에, 제임스 클리어(Atomic Habits)의 정체성 기반 변화 개념을 더해 Identity, Key Result Area, Income Generating Task, Key Performance Indicator를 한 흐름으로 관리합니다. 최상위 단계는 "나는 어떤 사람이 될 것인가?"라는 정체성 선언이며, 하위의 모든 실행과 측정은 그 정체성을 증명하는 과정입니다.

## IKIK 구조

```text
Identity 정체성
  -> Key Result Area 핵심 결과 영역
    -> Income Generating Task 소득 창출 업무
      -> Key Performance Indicator 핵심 성과 지표
```

## 기능 스캐폴딩

- IKIK 항목 생성, 조회, 수정, 삭제
- Identity, Key Result Area, Income Generating Task, Key Performance Indicator 단계별 보드
- 단계별 상위 항목 검증
- Key Performance Indicator 목표값, 현재값, 단위 저장
- Key Performance Indicator 측정값 기록을 위한 백엔드 유스케이스와 Tauri command
- SQLite 로컬 저장소
- Dioxus 프론트엔드와 Tauri 백엔드

## 프로젝트 구조

```text
ikik/
├── domain/                    # 엔티티와 리포지토리 인터페이스
├── application/               # IKIK 유스케이스와 검증 규칙
├── infrastructure/            # SQLite 구현체
├── presentation_backend/      # Tauri commands
├── presentation_frontend/     # Dioxus UI
├── Makefile.toml
└── Cargo.toml
```

## 개발

```shell
cargo check --workspace
cargo test --workspace
cargo tauri dev
```

프론트엔드만 확인하려면 다음을 사용할 수 있습니다. 이 모드에서는 Tauri IPC가 없어 CRUD 저장은 동작하지 않습니다.

```shell
./scripts/dev.sh
```
