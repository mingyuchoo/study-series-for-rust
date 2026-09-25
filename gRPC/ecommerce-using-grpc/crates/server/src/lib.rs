use proto::product_info_server::ProductInfo;
use proto::{Empty,
            Product,
            ProductId,
            ProductList};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32,
                        Ordering};
use std::sync::{Arc,
                Mutex};
use thiserror::Error;
use tonic::{Request,
            Response,
            Status};

// Define custom error types for Railway Oriented Programming
#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Product not found: {0}")]
    NotFound(i32),

    #[error("Invalid product data: {0}")]
    InvalidData(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

// Implement conversion from ServiceError to tonic::Status
impl From<ServiceError> for Status {
    fn from(error: ServiceError) -> Self {
        match error {
            | ServiceError::NotFound(id) => Status::not_found(format!("Product with id {} not found", id)),
            | ServiceError::InvalidData(msg) => Status::invalid_argument(msg),
            | ServiceError::Internal(msg) => Status::internal(msg),
        }
    }
}

// Type alias for service results to simplify Railway Oriented Programming
pub type ServiceResult<T> = Result<T, ServiceError>;

// Product service implementation with in-memory storage
#[derive(Debug)]
pub struct MyProductInfo {
    store: Arc<Mutex<HashMap<i32, Product>>>,
    next_id: Arc<AtomicI32>,
}

impl MyProductInfo {
    pub fn new() -> Self {
        Self {
            store: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicI32::new(1)),
        }
    }
}

impl Default for MyProductInfo {
    fn default() -> Self { Self::new() }
}

impl MyProductInfo {
    // Internal method to add a product with Railway Oriented Programming
    async fn add_product_internal(&self, product: Product) -> ServiceResult<ProductId> {
        // Validate product data
        if product.name.is_empty() {
            return Err(ServiceError::InvalidData("Product name cannot be empty".to_string()));
        }

        if product.price <= 0.0 {
            return Err(ServiceError::InvalidData("Product price must be positive".to_string()));
        }

        // Assign a new auto-incremented ID
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let stored = Product {
            id,
            ..product
        };

        self.store.lock().map_err(|e| ServiceError::Internal(e.to_string()))?.insert(id, stored);

        Ok(ProductId {
            id,
        })
    }

    // Internal method to get a product with Railway Oriented Programming
    async fn get_product_internal(&self, product_id: ProductId) -> ServiceResult<Product> {
        if product_id.id <= 0 {
            return Err(ServiceError::NotFound(product_id.id));
        }

        self.store
            .lock()
            .map_err(|e| ServiceError::Internal(e.to_string()))?
            .get(&product_id.id)
            .cloned()
            .ok_or(ServiceError::NotFound(product_id.id))
    }

    // Internal method to list all products
    async fn list_products_internal(&self) -> ServiceResult<ProductList> {
        let store = self.store.lock().map_err(|e| ServiceError::Internal(e.to_string()))?;
        let mut products: Vec<Product> = store.values().cloned().collect();
        products.sort_by_key(|p| p.id);
        Ok(ProductList {
            products,
        })
    }
}

#[tonic::async_trait]
impl ProductInfo for MyProductInfo {
    async fn add_product(&self, request: Request<Product>) -> Result<Response<ProductId>, Status> {
        let product = request.into_inner();
        self.add_product_internal(product).await.map(Response::new).map_err(Into::into)
    }

    async fn get_product(&self, request: Request<ProductId>) -> Result<Response<Product>, Status> {
        let product_id = request.into_inner();
        self.get_product_internal(product_id).await.map(Response::new).map_err(Into::into)
    }

    async fn list_products(&self, _request: Request<Empty>) -> Result<Response<ProductList>, Status> {
        self.list_products_internal().await.map(Response::new).map_err(Into::into)
    }
}
