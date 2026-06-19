<p align="center">
  <a href="https://github.com/mingyuchoo/rust-study-series/blob/main/LICENSE"><img alt="license" src="https://img.shields.io/github/license/mingyuchoo/rust-study-series"/></a>
  <a href="https://github.com/mingyuchoo/rust-study-series/issues"><img alt="Issues" src="https://img.shields.io/github/issues/mingyuchoo/rust-study-series?color=appveyor" /></a>
  <a href="https://github.com/mingyuchoo/rust-study-series/pulls"><img alt="GitHub pull requests" src="https://img.shields.io/github/issues-pr/mingyuchoo/rust-study-series?color=appveyor" /></a>
</p>

# Rust Study Series

Rust 언어를 활용한 다양한 주제별 학습 프로젝트 모음입니다.

## 프로젝트 구조

| 디렉터리 | 설명 | 하위 프로젝트 수 |
|-----------|------|:-----------------:|
| **ABI** | Node.js 네이티브 모듈 (NAPI-RS) | 1 |
| **AI** | AI/LLM 클라이언트, RAG, 에이전트 | 11 |
| **AWS** | AWS SDK, Lambda, SAM 앱 | 4 |
| **Azure** | Azure Bicep + Rust 앱 | 1 |
| **Books** | 서적 기반 학습 (The Rust Programming Language 등) | 1 |
| **CLI** | 커맨드라인 도구 (매니저, 파서, 크롤러 등) | 12 |
| **Cache** | Redis 캐싱 | 1 |
| **Database** | DB 연동 (Diesel, SQLx, MySQL, PostgreSQL, SQLite, Qdrant) | 9 |
| **GUI** | GUI 프레임워크 (Dioxus, egui, gpui, Iced, Tauri) | 5 |
| **JupyterLab** | Rust Jupyter 노트북 | 1 |
| **Patterns** | 디자인 패턴, 클린 아키텍처, 플러그인 아키텍처 | 3 |
| **TUI** | 터미널 UI (ratatui) | 1 |
| **WASI** | WebAssembly System Interface + gRPC | 2 |
| **WASM** | WebAssembly (Game of Life 등) | 3 |
| **Web** | 웹 프레임워크 (Actix, Axum, Loco, Kafka) | 4 |
| **gRPC** | gRPC 서비스 | 2 |

### NixOS

root 계정으로 `/etc/nixos/configuration.nix` 편집:

```nix
{ config, pkgs, ... }:
{
  users.users.{username} = {
  rustup
  }
}
```

사용자 계정에서 stable 툴체인 설치:

```bash
rustup default stable
rustup component add rls # or `llvm`
rustup component add rust-analysis
rustup component add rust-analyzer
```

사용자 계정에서 cargo 도구 설치:

```bash
cargo install cargo-audit
cargo install cargo-binstall
cargo install cargo-dist
cargo install cargo-edit // for upgrade Cargo.toml dependencies
cargo install cargo-expand
cargo install cargo-lambda
cargo install cargo-modules
cargo install cargo-udepts
cargo install cargo-deps
cargo install cargo-tree
cargo install cargo-watch
cargo install sccache
```

### Nix

```bash
sh <(curl -L https://nixos.org/nix/install) --daemon
# or
sh <(curl -L https://nixos.org/nix/install) --no-daemon

nix-channel --add https://nixos.org/channels/nixpkgs-unstable nixpkgs
nix-channel --update
```

#### Nix 개발 환경 실행

이 저장소에는 `flake.nix`가 포함되어 있어 `cargo`, `cargo-watch`, `clippy`, `rustfmt`, `rustup` 등이 자동으로 설정됩니다.

```bash
nix develop
```

### Ubuntu

```bash
sudo apt update
sudo apt install -y musl-tools
```

#### Rustup 설치

- <https://rustup.rs/>

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
rustup update stable
```

#### 타겟 아키텍처 설정

WSL2 (Windows 11, Snapdragon X Elite) 환경의 경우:

```bash
rustup update
rustup target add aarch64-unknown-linux-gnu
```

`$HOME/.cargo/config.toml` 생성:

```toml
# For Linux/macOS

# > cargo install mold
[target.x86_64-unknown-linux-gnu]
  linker    = "clang"
  rustflags = ["-C", "link-arg=-fuse-ld=mold"]

# For Windows
[target.x86_64-pc-windows-msvc]
  rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[build]
  target = "aarch64-unknown-linux-gnu"
  rustc-wrapper = "sccache"
```

프로젝트 빌드:

```bash
cargo build
cargo build --release
```

#### 컴포넌트 설치

```bash
rustup component list
rustup component add cargo
rustup component add clippy
rustup component add llvm-tools
rustup component add rls
rustup component add rust-analysis
rustup component add rust-analyzer
```

#### 링커 변경

`$HOME/.cargo/config.toml` 생성:

```toml
# For Linux/macOS-mold
# > cargo install mold
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

# For Windows-lld
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

## 포맷팅 및 린트

```bash
cargo fmt
cargo clippy --fix
```

## Watch 모드 사용법

### `cargo-watch` 설치

```bash
cargo install cargo-watch
```

### `cargo-watch`로 실행

```bash
# 테스트만 실행
cargo watch -x test

# check 후 테스트 실행
cargo watch -x check -x test

# src 변경 감시 + 콘솔 초기화 후 실행
cargo watch -c -w src -x run

# 특정 바이너리 실행
cargo watch -x 'run --bin app'

# 인자 전달하여 실행
cargo watch -x 'run -- --some-arg'

# 예제 파일 실행
cargo watch -q -c -x "run -q --example c01-simple"

# 임의 명령어 실행
cargo watch -- echo Hello world

# feature 플래그와 함께 실행
cargo watch --features "foo,bar"
```

## 모듈 구조 확인

### `cargo-modules` 설치

```bash
cargo install cargo-modules
```

### 크레이트 모듈 구조 조회

```bash
cargo modules generate tree --types
```

## 의존성 업그레이드

```bash
cargo install cargo-edit
cargo install cargo-outdated
cargo outdated
cargo upgrade
cargo build
cargo test
```
