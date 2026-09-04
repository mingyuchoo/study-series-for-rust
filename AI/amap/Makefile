.PHONY: architecture build test demo lint wasm web web-test services infra
architecture: ; python3 scripts/check_architecture.py
build:       ; cargo build --workspace --locked
test:        ; cargo test --workspace --locked
lint:        ; cargo clippy --workspace --all-targets --locked -- -D warnings
demo:        ; cargo run --locked -q --bin amap -- demo
wasm:        ; cargo build --release --target wasm32-unknown-unknown --manifest-path plugins/finance/Cargo.toml --locked
web:         ; npm --prefix web run build
web-test:    ; npm --prefix web test
infra:       ; docker compose up -d
control-plane: ; cargo run --bin control-plane
llm-gateway: ; cargo run --bin llm-gateway
worker:      ; cargo run --bin verification-worker
