use anyhow::{Context,
             Result};
use proto::product_info_server::ProductInfoServer;
use server::MyProductInfo;
use std::net::SocketAddr;
use tonic::transport::Server;
use tracing::{Level,
              info};
use tracing_subscriber::FmtSubscriber;

/// Server configuration
struct ServerConfig {
    addr: SocketAddr,
}

impl ServerConfig {
    /// Create a new server configuration with default settings
    fn new() -> Result<Self> {
        let addr = "[::1]:50051".parse().context("Failed to parse server address")?;

        Ok(Self {
            addr,
        })
    }
}

/// Initialize the tracing subscriber for logging
fn init_tracing() -> Result<()> {
    let subscriber = FmtSubscriber::builder().with_max_level(Level::INFO).finish();

    tracing::subscriber::set_global_default(subscriber).context("Failed to set tracing subscriber")?;

    Ok(())
}

/// Start the gRPC server with the provided configuration
async fn start_server(config: ServerConfig) -> Result<()> {
    // Setup gRPC standard health checking service
    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<ProductInfoServer<MyProductInfo>>().await;

    // Create the product service implementation
    let product_service = MyProductInfo::default();

    // Log server startup
    info!("ProductInfoServer listening on {}", config.addr);

    // Build and start the server
    Server::builder()
        .add_service(health_service)
        .add_service(ProductInfoServer::new(product_service))
        .serve(config.addr)
        .await
        .context("Server failed")?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for better logging
    init_tracing()?;

    // Create server configuration
    let config = ServerConfig::new()?;

    // Start the server with Railway Oriented Programming pattern
    start_server(config).await
}
