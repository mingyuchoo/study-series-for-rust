use backend::infrastructure::{AppState, Config, create_app, create_database_pool, initialize_database};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = Config::from_args();

    // Initialize database
    let pool = match create_database_pool().await {
        | Ok(pool) => pool,
        | Err(e) => {
            eprintln!("Failed to connect to database: {}", e);
            eprintln!("Make sure the current directory is writable");
            std::process::exit(1);
        },
    };

    // Initialize database schema
    if let Err(e) = initialize_database(&pool).await {
        eprintln!("Failed to initialize database: {}", e);
        std::process::exit(1);
    }

    // Setup dependency injection
    let app_state = AppState::new(pool);

    // Start the server
    create_app(app_state, config.port).await
}
