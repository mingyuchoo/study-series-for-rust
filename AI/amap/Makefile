.PHONY: build test demo lint wasm services infra
build:       ; cargo build --workspace --locked
test:        ; cargo test --workspace --locked
lint:        ; cargo clippy --workspace --all-targets --locked -- -D warnings
demo:        ; cargo run --locked -q --bin amap -- demo
wasm:        ; cargo build --release --target wasm32-unknown-unknown --manifest-path plugins/finance/Cargo.toml --locked
infra:       ; docker compose up -d
control-plane: ; cargo run --bin control-plane
llm-gateway: ; cargo run --bin llm-gateway
worker:      ; cargo run --bin verification-worker
