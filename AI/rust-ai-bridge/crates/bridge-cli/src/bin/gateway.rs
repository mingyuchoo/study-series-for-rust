//! MCP 게이트웨이 진입점 (stdio 또는 Streamable HTTP) + 운영 콘솔 · 메트릭.

use ai_bridge::mcpserver;
use anyhow::Result;
use bridge_cli::bootstrap::{self,
                            Config};
use clap::Parser;
use rmcp::{ServiceExt,
           transport::{stdio,
                       streamable_http_server::{StreamableHttpService,
                                                session::local::LocalSessionManager}}};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::parse();

    // stdio 전송은 stdout 이 MCP 채널이므로 **로그는 stderr 로** 나가야 합니다.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let http_addr = cfg.http.clone();
    let b = bootstrap::bootstrap(cfg).await?;

    let console = bootstrap::serve_console(&b).await?;
    let metrics = bootstrap::serve_metrics(&b).await?;
    bootstrap::start_signal_handlers(b.ops.clone());

    let server = mcpserver::Server::new(b.gateway.clone(), b.resolver.clone(), b.resources.clone());

    match http_addr {
        // --- Streamable HTTP — 요청마다 주체를 해석합니다 ---
        | Some(addr) => {
            let addr = match addr.strip_prefix(':') {
                | Some(port) => format!("0.0.0.0:{port}"),
                | None => addr,
            };
            tracing::info!("MCP (Streamable HTTP): http://{addr}");

            let service = StreamableHttpService::new(move || Ok(server.clone()), Arc::new(LocalSessionManager::default()), Default::default());
            let app = axum::Router::new().fallback_service(service);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            axum::serve(listener, app).await?;
        },

        // --- stdio — 프로세스 하나가 사용자 한 명을 대신합니다 ---
        | None => {
            tracing::info!("MCP (stdio)");
            let running = server.serve(stdio()).await?;
            running.waiting().await?;
        },
    }

    for h in [console, metrics].into_iter().flatten() {
        h.abort();
    }
    Ok(())
}
