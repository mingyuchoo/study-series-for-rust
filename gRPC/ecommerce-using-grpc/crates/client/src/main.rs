use anyhow::{Context,
             Result};
use proto::product_info_client::ProductInfoClient;
use proto::{Product,
            ProductId};
use tonic::Request;
use tonic::transport::Channel;
use tracing::{Level,
              info};
use tracing_subscriber::FmtSubscriber;

/// Client configuration
struct ClientConfig {
    server_url: String,
}

impl ClientConfig {
    /// Create a new client configuration with default settings
    fn new() -> Self {
        Self {
            server_url: "http://[::1]:50051".to_string(),
        }
    }

    /// Create a client with custom server URL
    #[allow(dead_code)]
    fn with_server_url(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
        }
    }
}

/// Initialize the tracing subscriber for logging
fn init_tracing() -> Result<()> {
    let subscriber = FmtSubscriber::builder().with_max_level(Level::INFO).finish();

    tracing::subscriber::set_global_default(subscriber).context("Failed to set tracing subscriber")?;

    Ok(())
}

/// Connect to the gRPC server
async fn connect_to_server(config: &ClientConfig) -> Result<ProductInfoClient<Channel>> {
    ProductInfoClient::connect(config.server_url.clone())
        .await
        .context("Failed to connect to server")
}

/// Get a product by ID
async fn get_product(client: &mut ProductInfoClient<Channel>, id: i32) -> Result<Product> {
    // Create the request
    let request = Request::new(ProductId {
        id,
    });

    // Send the request and handle the response with Railway Oriented
    // Programming
    let response = client.get_product(request).await.context("Failed to get product")?;

    // Extract the product from the response
    Ok(response.into_inner())
}

/// Add a new product
async fn add_product(client: &mut ProductInfoClient<Channel>, product: Product) -> Result<i32> {
    // Create the request
    let request = Request::new(product);

    // Send the request and handle the response with Railway Oriented
    // Programming
    let response = client.add_product(request).await.context("Failed to add product")?;

    // Extract the product ID from the response
    Ok(response.into_inner().id)
}

/// Display product information
fn display_product(product: &Product) {
    info!("Product Information:");
    info!("ID: {}", product.id);
    info!("Name: {}", product.name);
    info!("Description: {}", product.description);
    info!("Price: ${:.2}", product.price);
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for better logging
    init_tracing()?;

    // Create client configuration
    let config = ClientConfig::new();

    // Connect to the server
    let mut client = connect_to_server(&config).await?;

    // Create a sample product
    let new_product = Product {
        id: 2,
        name: String::from("Google Pixel 7"),
        description: String::from("The latest Google Pixel smartphone"),
        price: 599.99,
    };

    // Add the product
    let product_id = add_product(&mut client, new_product).await?;
    info!("Added product with ID: {}", product_id);

    // Get a product
    let product = get_product(&mut client, 1).await?;

    // Display the product information
    display_product(&product);

    Ok(())
}
