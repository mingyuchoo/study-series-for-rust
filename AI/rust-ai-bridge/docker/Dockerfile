# rust-ai-bridge 이미지 (gateway · auditctl · agentctl · evalctl)
#
# 빌드는 리포지토리 루트를 컨텍스트로 삼습니다:
#   docker build -f docker/Dockerfile -t rust-ai-bridge:latest .
# 또는 scripts/docker.sh build

# --- 빌드 단계 ---------------------------------------------------------------
# rust-toolchain.toml 이 nightly 를 고정하므로 nightly 이미지를 씁니다.
FROM rustlang/rust:nightly-bookworm-slim AS builder

WORKDIR /app

# 툴체인을 먼저 확정해 두면 소스가 바뀌어도 이 레이어는 재사용됩니다.
COPY rust-toolchain.toml ./
RUN rustup show

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# registry·target 캐시 마운트로 재빌드를 줄입니다. 캐시는 이미지에 남지 않으므로
# 산출물을 같은 RUN 안에서 /out 으로 복사해야 합니다.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo build --locked --workspace --release \
    && mkdir -p /out \
    && cp target/release/gateway /out/ \
    && cp target/release/auditctl /out/ \
    && cp target/release/agentctl /out/ \
    && cp target/release/evalctl /out/

# --- 실행 단계 ---------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# ca-certificates: 외부 백엔드·LLM 호출의 TLS 검증에 필요합니다.
# curl: 컴포즈 healthcheck(/readyz)에 씁니다.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --uid 10001 --home-dir /app --shell /usr/sbin/nologin app

WORKDIR /app

COPY --from=builder /out/gateway /out/auditctl /out/agentctl /out/evalctl /usr/local/bin/

# 정책·주체·시스템 카탈로그와 평가 스위트는 CWD 기준 상대 경로로 읽힙니다(= /app).
COPY config ./config
COPY eval ./eval

# SQLite 감사 로그 등 쓰기가 필요한 상태는 전부 여기에만 둡니다.
RUN mkdir -p /app/data && chown -R app:app /app/data

USER app

ENV RUST_LOG=info

# 8080 MCP(Streamable HTTP) · 8081 운영 콘솔 · 9464 Prometheus 메트릭
EXPOSE 8080 8081 9464

# 기본값은 안전한 쪽입니다. systems.yaml 이 mock/memory 백엔드인 개발 구성이라면
# --allow-mock-backends 를 인자로 넘겨야 기동합니다(scripts/docker.sh 가 붙여 줍니다).
CMD ["gateway", \
     "--db", "/app/data/audit.db", \
     "--http", ":8080", \
     "--console", ":8081", \
     "--metrics", ":9464"]
