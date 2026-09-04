FROM rust:1.98-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --workspace --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates python3 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/amap /src/target/release/control-plane /src/target/release/llm-gateway /src/target/release/verification-worker /usr/local/bin/
COPY config /app/config
COPY migrations /app/migrations
WORKDIR /app
ENTRYPOINT ["control-plane"]
