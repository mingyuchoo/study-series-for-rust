# VVKIK

Clean Architecture 기반의 개인 목표와 성과 관리 데스크톱 앱입니다. Rob Moore의 큰 그림에서 세부 실행으로 내려가는 모델을 바탕으로 Value, Vision, KRA, IGT, KPI를 한 흐름으로 관리합니다.

## VVKIK 구조

```text
Value 가치
  -> Vision 비전
    -> KRA 핵심 결과 영역
      -> IGT 소득 창출 업무
      -> KPI 핵심 성과 지표
```

## 기능 스캐폴딩

- VVKIK 항목 생성, 조회, 수정, 삭제
- Value, Vision, KRA, IGT, KPI 단계별 보드
- 단계별 상위 항목 검증
- KPI 목표값, 현재값, 단위 저장
- KPI 측정값 기록을 위한 백엔드 유스케이스와 Tauri command
- SQLite 로컬 저장소
- Dioxus 프론트엔드와 Tauri 백엔드

## 프로젝트 구조

```text
vvkik/
├── domain/                    # 엔티티와 리포지토리 인터페이스
├── application/               # VVKIK 유스케이스와 검증 규칙
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
