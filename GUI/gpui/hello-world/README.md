# hello-world

Create GPUI App으로 생성된 기본 gpui 예제 프로젝트입니다.

## 주요 의존성

- **gpui-pre 0.3.7** (`gpui` 이름으로 사용): GPU 가속 UI 프레임워크
- **gpui-pre-platform 0.3.7** (`gpui-platform` 이름으로 사용): OS별 앱 초기화 및 렌더링 백엔드
- **gpui-component 0.7.0**: gpui 컴포넌트 라이브러리
- **anyhow 1.0.104**: 에러 처리

`gpui-component 0.7.0`이 사용하는 `gpui-pre =0.3.7`과 동일한 버전을 지정하여
GPUI 타입 호환성을 유지합니다. 앱 초기화는 최신 API인
`gpui_platform::application()`을 사용합니다. `Cargo.lock`은 직접·간접 의존성의 검증된 버전을
기록하며 Git으로 관리합니다.

## 실행 방법

- Rust가 설치되어 있어야 합니다 - [Rustup](https://rustup.rs/)
- `cargo run --locked` 명령으로 실행

Windows 11의 Windows PowerShell 5.1 또는 PowerShell 7에서는 다음 스크립트로
정리 → 빌드 → 테스트 → 실행을 순서대로 수행할 수 있습니다.

```powershell
.\scripts\run.ps1
```

스크립트는 어느 디렉터리에서 호출하더라도 프로젝트 루트에서 실행하며,
종료 시 호출한 디렉터리로 복귀합니다. Cargo 단계가 실패하면 즉시 중단하고
해당 종료 코드를 반환합니다. 추가 인자는 `cargo run`의 `--` 뒤로 전달됩니다.

## 의존성 갱신 및 검증

직접 의존성의 새 버전은 루트 `Cargo.toml`의 `[workspace.dependencies]`에서
갱신합니다. GPUI 버전은 `gpui-component`이 요구하는 버전과 함께 맞춥니다.
간접 의존성은 각 crate가 허용하는 범위 내에서 최신 버전으로 갱신합니다.

```sh
cargo update
cargo fmt --all -- --check
cargo build --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## 참고 자료

- [`gpui`](https://www.gpui.rs/)
- [GPUI 문서](https://github.com/zed-industries/zed/tree/main/crates/gpui/docs)
- [GPUI 예제](https://github.com/zed-industries/zed/tree/main/crates/gpui/examples)
