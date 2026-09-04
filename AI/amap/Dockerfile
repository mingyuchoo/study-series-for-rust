FROM node:24-bookworm-slim AS web-build
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web ./
COPY openapi /src/openapi
RUN npm run build

FROM rust:1.98-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --workspace --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates python3 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/amap /src/target/release/control-plane /src/target/release/llm-gateway /src/target/release/verification-worker /usr/local/bin/
COPY --from=web-build /src/web/dist /app/web/dist
COPY config /app/config
COPY migrations /app/migrations
WORKDIR /app
ENTRYPOINT ["control-plane"]
