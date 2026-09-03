.PHONY: build test demo lint wasm services infra
build:       ; cargo build --workspace
test:        ; cargo test --workspace
lint:        ; cargo clippy --workspace --all-targets -- -D warnings
demo:        ; cargo run -q --bin amap -- demo
wasm:        ; cargo build --release --target wasm32-unknown-unknown --manifest-path plugins/finance/Cargo.toml
infra:       ; docker compose up -d
control-plane: ; cargo run --bin control-plane
llm-gateway: ; cargo run --bin llm-gateway
worker:      ; cargo run --bin verification-worker
