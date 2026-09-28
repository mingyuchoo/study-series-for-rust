mod web;

use std::env;
use web::{AppState,
          router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let brokers = env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into());
    let address = env::var("WEB_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let frontend = env::var("FRONTEND_DIST").unwrap_or_else(|_| "frontend/dist".into());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    let state = AppState::new(brokers)?;
    let shutdown_state = state.clone();
    println!("Kafka web server: http://{}", listener.local_addr()?);
    axum::serve(listener, router(state, &frontend))
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            shutdown_state.shutdown();
        })
        .await?;
    Ok(())
}
