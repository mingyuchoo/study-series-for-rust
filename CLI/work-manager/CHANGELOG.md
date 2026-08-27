# 변경 이력

이 프로젝트의 주요 변경 사항을 기록합니다.
형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/)를 따르며,
버전은 [유의적 버전](https://semver.org/lang/ko/)을 따릅니다.

## [Unreleased]

아직 릴리스된 버전이 없습니다. 아래 내용이 첫 릴리스 `0.1.0`이 됩니다.

### Added

#### 1단계 — 격리된 개발 공간

- `wm dev new <name> [--base <ref>]` — `.wm/worktrees/<name>`에 git worktree와
  동명 브랜치를 만듭니다.
- `wm dev list` — worktree 목록. 현재 서 있는 곳을 `*`로 표시합니다.
- `wm dev rm <name> [--force]` — worktree 제거. 브랜치는 남깁니다.
- `wm dev run [--from N] [--no-build] [--detach]` — worktree 코드를 그 자리에서
  실행합니다. 세대를 만들지 않고, 전경으로 붙어 실행하며(`--detach`로 배경),
  빌드 단계는 돌리되 테스트·헬스체크·자동 롤백은 하지 않습니다. 실행한 코드의
  종료 코드를 그대로 전달합니다.
- worktree 안에서 실행한 명령은 **원본 프로젝트 루트를 기준으로** 동작합니다.
  worktree에도 매니페스트 복사본이 있지만 별도의 세대 저장소가 생기지 않으며,
  덕분에 `wm build`와 `wm dev run`이 대상 플래그 없이 현재 worktree를 씁니다.

#### 2단계 — 빌드·테스트·세대

- `wm build [--from N] [--note T] [--switch]` — 빌드와 테스트를 거쳐 산출물을
  불변 세대로 동결합니다. 어느 단계든 실패하면 세대 번호를 소비하지 않습니다.
- `wm switch [--gen N]` — 세대를 활성화하고 헬스체크로 검증합니다.
- 세대 저장소는 `.wm/store/NNNN-<sha>`, 번호 링크는 `.wm/generations/NNNN`,
  활성 포인터는 `.wm/current`입니다. 전환은 임시 심볼릭 링크 생성 후
  `rename(2)`이라 원자적입니다.
- 산출물은 링크가 아니라 복사합니다. 이후 개발이 과거 세대를 바꾸지 못합니다.
- 헬스체크 프로브 `tcp` / `http` / `cmd`. 여러 개를 두면 전부 통과해야 합니다.

#### 3단계 — 롤백

- `wm rollback [--to N]` — 이전 세대로 복귀합니다. 기본값은 활성 세대 바로
  아래 번호입니다.
- **자동 롤백** — 헬스체크에 실패하면 해당 세대를 `rejected`로 기록하고 이전
  세대를 복원한 뒤, 그 세대가 다시 응답할 때까지 기다렸다가 종료 코드 `1`을
  반환합니다.

#### 실행과 상태

- 프로젝트당 서비스 슬롯은 **하나**입니다. 슬롯을 차지한 주체를
  `.wm/run/state.json`에 pid와 함께 기록하므로, `wm status`가 활성 세대인지
  개발용 worktree인지 이름을 댈 수 있습니다.
- 슬롯을 가져가는 쪽은 무엇을 밀어냈는지 출력합니다.
- `wm start` / `wm stop` / `wm restart` / `wm logs [-n N]` — 활성 세대의
  프로세스 제어. 배경 실행은 `setsid`로 분리해 `wm`을 Ctrl-C 해도 살아남고,
  종료는 SIGTERM 후 유예 시간을 두고 SIGKILL입니다.
- `wm status` — 활성 세대, 서 있는 worktree, 실행 중인 것의 출처.

#### 그 밖에

- `wm init [--preset P] [--name N] [--force]` — 매니페스트를 생성합니다.
  프리셋은 `rust` / `node` / `python` / `go` / `generic`이며, 미지정 시
  프로젝트 파일을 보고 고릅니다.
- `wm generations` (별칭 `gens`) — 세대 목록과 상태(`built` / `healthy` /
  `rejected`), 크기.
- `wm history` — 전환 이력과 사유 (`.wm/history.jsonl`).
- `wm gc [--keep N]` — 오래된 세대 삭제(기본 5). 활성 세대와 롤백 대상은 항상
  보존합니다.
- 전역 옵션 `-C, --directory <DIR>`.
- 프로젝트 단위 배타 잠금 — 상태를 바꾸는 명령이 동시에 실행되지 않습니다.
- 빌드·테스트·실행이 전부 shell 커맨드라 언어에 중립적입니다.

### 알려진 제약

- Unix 계열 전용입니다 (`setsid(2)`, 심볼릭 링크, `rename(2)`).
- 재현 가능한 빌드를 보장하지 않습니다. 커밋 해시와 dirty 여부를 기록할 뿐입니다.
- 로컬 프로세스 하나만 관리합니다. 다중 서비스와 재부팅 후 복구는 범위 밖입니다.
- 헬스체크 HTTP는 평문 전용이며, 프로세스 생존 자체는 확인하지 않습니다.

---

## 기록 방침

- 사용자에게 보이는 변화만 적습니다. 리팩터링, 내부 구조 변경, 같은 릴리스
  주기 안에서 만들어졌다가 고쳐진 버그는 넣지 않습니다.
- 릴리스할 때는 `[Unreleased]`의 내용을 `## [0.1.0] - YYYY-MM-DD`로 옮기고,
  `Cargo.toml`의 `workspace.package.version`과 git 태그를 맞춥니다.
