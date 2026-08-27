# work-manager (`wm`)

개발 라이프사이클을 **worktree → 세대(generation) → 롤백** 세 단계로 관리하는 CLI.

NixOS의 세대/롤백 모델을 일반 프로젝트에 옮긴 것입니다. 다만 입력 해시 기반
재현성은 목표로 하지 않고, **번호가 매겨진 불변 스냅샷 + 원자적 심볼릭 링크 전환**
까지만 보장합니다.

```
1단계  wm dev new <name>            원본을 건드리지 않는 격리된 worktree
       wm dev run                   worktree 코드를 그 자리에서 실행 (개발 내부 루프)
2단계  wm build --switch            빌드 → 테스트 → 세대 생성 → 활성화 → 헬스체크
3단계  wm rollback                  이전 세대로 복귀
```

헬스체크가 실패하면 `wm switch`가 **자동으로 이전 세대를 되돌리고** 종료 코드 1을
반환합니다. 이 자동 롤백이 도구의 핵심이며, 단순 배포 스크립트와의 차이입니다.

변경 이력은 [CHANGELOG.md](CHANGELOG.md)를 참고하세요.

## 요구 사항

- **Unix 계열 전용.** 프로세스 분리에 `setsid(2)`, 세대 전환에 심볼릭 링크와
  `rename(2)`을 씁니다. Windows는 지원하지 않습니다.
- **git.** `wm dev *` 명령이 `git worktree`를 직접 호출합니다. 그 외 명령은
  git 없이도 동작합니다.
- **Rust 2024 edition** 지원 툴체인.

## 설치

```bash
cargo build --release          # target/release/wm
cargo install --path crates/wm-cli
```

## 빠른 시작

```bash
wm init --preset rust          # work-manager.toml 생성 (미지정 시 자동 감지)

wm dev new add-cache           # 1단계: .wm/worktrees/add-cache + 동명 브랜치
cd .wm/worktrees/add-cache     #        개발
wm dev run                     #        그 자리에서 실행 (전경, 검증 없음)
wm build --switch              # 2단계: 빌드·테스트·활성화·검증

wm generations                 # 세대 목록 (* 가 활성 세대)
wm status                      # 활성 세대 + 지금 무엇이 돌고 있는지
wm logs -n 100                 # 서비스 로그

wm rollback                    # 3단계: 바로 이전 세대로
wm gc --keep 5                 # 오래된 세대 정리
```

## 명령어

모든 명령에 전역 옵션 `-C, --directory <DIR>`을 쓸 수 있습니다(해당 디렉토리에서
시작한 것처럼 동작).

| 명령 | 설명 |
| --- | --- |
| `wm init [--preset P] [--name N] [--force]` | 매니페스트 생성. 프리셋 미지정 시 프로젝트 파일로 자동 감지 |
| `wm status` | 활성 세대, 서 있는 worktree, 실행 중인 것의 **출처** |
| `wm dev new <name> [--base <ref>]` | worktree + 동명 브랜치 생성 |
| `wm dev list` | worktree 목록 (`*` = 현재 서 있는 곳) |
| `wm dev run [--from N] [--no-build] [--detach]` | worktree 코드를 실행. 세대를 만들지 않음 |
| `wm dev rm <name> [--force]` | worktree 제거 (브랜치는 남김) |
| `wm build [--from N] [--note T] [--switch]` | 빌드·테스트 후 세대 생성. `--switch`면 이어서 활성화 |
| `wm switch [--gen N]` | 세대 활성화 + 검증 (기본: 가장 최근 세대) |
| `wm rollback [--to N]` | 이전 세대로 복귀 (기본: 활성 세대 바로 아래) |
| `wm generations` (`gens`) | 세대 목록. 상태는 `built` / `healthy` / `rejected` |
| `wm history` | 전환 이력과 사유 |
| `wm gc [--keep N]` | 오래된 세대 삭제 (기본 5). 활성 세대와 롤백 대상은 항상 보존 |
| `wm start` / `wm stop` / `wm restart` | 활성 세대의 프로세스 제어 |
| `wm logs [-n N]` | 서비스 로그 꼬리 (기본 40줄) |

**종료 코드.** 검증에 실패해 자동 롤백된 `wm switch`(및 `wm build --switch`)는
`1`을 반환합니다. `wm dev run`은 실행한 코드의 종료 코드를 그대로 전달합니다.

### worktree 안에서의 동작

`work-manager.toml`은 커밋되어 있으므로 worktree에도 복사본이 있지만, `wm`은
`.wm/worktrees/<name>` 경로를 인식해 **항상 원본 프로젝트 루트를 기준으로**
동작합니다. worktree 안에 별도의 세대 저장소가 생기지 않습니다.

덕분에 worktree 안에서는 대상을 생략할 수 있습니다.

```bash
cd .wm/worktrees/add-cache
wm status        # 원본의 활성 세대가 보임 + `worktree add-cache (you are here)`
wm dev run       # 이 worktree를 실행
wm build         # 이 worktree를 빌드 (--from 불필요)
```

## 실행 슬롯과 실행 출처

한 프로젝트는 **하나의 서비스 슬롯**만 가집니다. 슬롯을 차지한 주체는
`.wm/run/state.json`에 기록되므로, `wm status`는 "뭔가 돌고 있다"가 아니라
**무엇이 돌고 있는지**를 답합니다.

```
  service      running (pid 15562, background)
  running      worktree `add-cache` (dev, unverified)   ← 세대가 아니라 개발 코드
```

| | `wm start` / `wm switch` | `wm dev run` |
| --- | --- | --- |
| 대상 | 활성 세대 (검증됨) | worktree (미검증) |
| 모드 | 배경 (setsid 분리) | 전경 (`--detach`로 배경) |
| 테스트 | 세대 생성 시 통과 | 건너뜀 |
| 헬스체크·자동 롤백 | 함 | **안 함** |
| 종료 | `wm stop` | Ctrl-C |

슬롯을 뺏을 때는 양쪽 모두 무엇을 밀어냈는지 출력합니다.

```
$ wm switch --gen 1
  → stopping worktree `add-cache` (dev, unverified) to take the service slot
```

## 매니페스트

`work-manager.toml`. 빌드·테스트·실행이 전부 shell 커맨드(`sh -c`)라 언어에
중립적입니다.

```toml
[project]
name = "demo"

[build]
cmd = "cargo build --release"
env = { RUSTFLAGS = "-C target-cpu=native" }   # 선택

[test]
cmd = "cargo test"                             # 생략하거나 비우면 건너뜀

[artifacts]
include = ["target/release/demo", "config"]    # 세대로 동결할 경로

[run]
cmd = "./demo"
env = { RUST_LOG = "info" }                    # 선택
stop_timeout_secs = 10                         # SIGTERM 후 SIGKILL까지 대기 (기본 10)

[health]
tcp = "127.0.0.1:8080"
timeout_secs = 30                              # 기본 30
interval_secs = 1                              # 기본 1
```

### 실행 디렉토리

`run.cmd`(그리고 `health.cmd`)의 작업 디렉토리는 다음과 같습니다.

- `wm start` / `wm switch` → 세대의 payload (`.wm/store/NNNN-xxx/root`)
- `wm dev run` → 해당 worktree 루트

payload는 **빌드 디렉토리의 상대 경로 구조를 그대로 복사**한 것이라, 두 경우에
같은 명령이 성립합니다. 따라서 `run.cmd`는 빌드 디렉토리 기준 상대 경로로
쓰면 되고, 실행 모드별로 설정을 나눌 필요가 없습니다.

`artifacts.include`의 항목도 빌드 디렉토리 기준 상대 경로여야 하며, `..`로
바깥을 가리킬 수 없습니다.

### 헬스체크

세 가지 프로브를 쓸 수 있고, **여러 개를 두면 전부 통과해야** 정상으로 봅니다.

| 키 | 의미 |
| --- | --- |
| `tcp = "host:port"` | TCP 연결 성공 |
| `http = "http://host:port/path"` | 2xx/3xx 응답 (평문 HTTP 전용) |
| `cmd = "..."` | 종료 코드 0 |

`timeout_secs` 안에 통과하지 못하면 활성화를 되돌립니다. `[health]`를 아예
생략하면 검증 없이 활성화만 하며, **자동 롤백도 일어나지 않습니다.**

### 프리셋

`wm init --preset <name>`. 미지정 시 `Cargo.toml` / `package.json` / `go.mod` /
`pyproject.toml`(또는 `requirements.txt`)을 보고 고릅니다.

`rust` · `node` · `python` · `go` · `generic`

프리셋은 출발점일 뿐이므로, 생성된 매니페스트의 명령과 산출물 경로는 프로젝트에
맞게 손봐야 합니다.

## 상태 레이아웃

파일시스템이 유일한 진실의 원천입니다. 데이터베이스는 없습니다.

```
.wm/
├── worktrees/<name>/            개발용 git worktree
├── store/0007-a1b2c3d/          불변 산출물(root/) + meta.json
├── generations/0007 -> ../store/0007-a1b2c3d
├── current -> generations/0007  활성 포인터 (rename(2)으로 원자적 교체)
├── history.jsonl                전환 감사 로그
├── run/state.json               실행 중인 pid + 그 출처(세대/worktree)
├── run/service.log
└── lock                         프로젝트 단위 배타 잠금
```

`current` 교체는 임시 심볼릭 링크 생성 후 `rename(2)`입니다. `ln -sfn`은
unlink와 symlink 사이에 활성 세대가 없는 구간이 생기므로 쓰지 않습니다.

`.wm/`은 프로젝트의 `.gitignore`에 넣으세요.

## 크레이트 구성

의존 방향은 `wm-cli → wm-runner → wm-store → wm-core` 단방향입니다.

| 크레이트 | 역할 |
| --- | --- |
| `wm-core` | 도메인 타입(`Config`, `Generation`, `RunSource`, `Error`). I/O 없음 |
| `wm-store` | 세대 저장소, 원자적 전환, GC, 파일 잠금 |
| `wm-runner` | git worktree, 빌드 실행, 산출물 수집, supervisor, 헬스체크 |
| `wm-cli` | `wm` 바이너리 |

## 설계상의 선택과 한계

- **재현성 아님.** 커밋 해시와 dirty 여부를 기록할 뿐, 같은 입력이 같은 출력을
  낸다고 보장하지 않습니다. 그건 Nix의 일입니다.
- **로컬 프로세스 하나.** supervisor는 상태 파일 하나, 로그 파일 하나입니다.
  재부팅 후 복구나 다중 서비스는 범위 밖이며, `Supervisor`가 systemd 백엔드를
  붙일 자리입니다.
- **산출물은 복사.** worktree에서 이후 개발을 해도 과거 세대가 바뀌지 않도록
  하드링크가 아닌 복사를 씁니다. 산출물이 크면 reflink 도입 여지가 있습니다.
- **git은 CLI 호출.** `git worktree` 지원은 libgit2 쪽이 어정쩡해서 `git`을
  직접 실행합니다.
- **`wm dev run`은 검증하지 않습니다.** 아직 안 되는 코드를 돌려보는 게 목적이라
  헬스체크와 자동 롤백을 일부러 뺐습니다. 검증이 필요하면 세대를 만드세요.
- **헬스체크 HTTP는 평문 전용.** 대상이 로컬 프로세스라 TLS 스택을 넣지
  않았습니다. `https://`는 `tcp` 또는 `cmd` 프로브로 대체하세요.
- **헬스체크는 프로세스 생존을 보지 않습니다.** `cmd = "true"`처럼 항상 통과하는
  프로브는 프로세스가 이미 죽었어도 정상으로 판정합니다. 실제 서비스 상태를
  보는 프로브를 쓰세요.

## 테스트

```bash
cargo test --workspace
```

라이브러리 단위 테스트와, 실제 `wm` 바이너리를 구동하는 CLI 통합 테스트로 나뉩니다.

| 대상 | 내용 |
| --- | --- |
| `wm-core` | worktree 경로 판별 |
| `wm-store` | 세대 번호·원자적 전환·롤백 대상 선택·GC 보호·잠금 |
| `wm-runner` | 헬스체크 URL 파싱 |
| `tests/worktree.rs` | `init`, `dev new/list/rm`, worktree 안에서의 발견 동작 |
| `tests/dev_run.rs` | `dev run` — 전경 실행, 종료 코드, 빌드 단계, 대상 자동 선택, 슬롯 인계 |
| `tests/lifecycle.rs` | `build`, `switch`, `rollback`, `generations`, `history`, `gc` |
| `tests/service.rs` | `start`, `stop`, `restart`, `logs`, `status`, `-C` |

통합 테스트는 임시 디렉토리에 프로젝트를 만들고 실제 프로세스를 띄웁니다.
git이 필요한 것은 `tests/worktree.rs`뿐입니다.
