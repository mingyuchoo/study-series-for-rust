use crate::domain::{AuthParams,
                    AuthRepository};
use anyhow::Result;
use std::sync::Arc;

pub struct AuthUseCases {
    repository: Arc<dyn AuthRepository + Send + Sync>,
}

impl AuthUseCases {
    pub fn new(repository: Arc<dyn AuthRepository + Send + Sync>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn sign_up(&self) -> Result<String> { self.repository.sign_up().await }

    pub async fn sign_in(&self, username: String, password: String) -> Result<String> {
        let params = AuthParams {
            name: username,
            pass: password,
        };
        self.repository.sign_in(params).await
    }

    pub async fn sign_in_root(&self) -> Result<String> { self.repository.sign_in_root().await }

    pub async fn get_session(&self) -> Result<String> { self.repository.get_session().await }
}
