use crate::app_error::AppResult;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use secrecy::{ExposeSecret,
              SecretString};
use std::sync::Arc;
use tracing::{info,
              instrument};
use uuid::Uuid;
#[async_trait]
pub trait UserPersistence: Send + Sync {
    async fn create_user(&self, username: &str, email: &str, password_hash: &str) -> AppResult<()>;
    async fn list_users(&self) -> AppResult<Vec<UserListItem>>;
    async fn get_user(&self, id: Uuid) -> AppResult<Option<UserDetail>>;
    async fn update_user(&self, id: Uuid, username: Option<&str>, email: Option<&str>, password_hash: Option<&str>) -> AppResult<()>;
    async fn delete_user(&self, id: Uuid) -> AppResult<()>;
}

pub trait UserCredentialsHasher: Send + Sync {
    fn hash_password(&self, password: &str) -> AppResult<String>;
}

#[derive(Clone)]
pub struct UserUseCases {
    hasher: Arc<dyn UserCredentialsHasher>,
    persistence: Arc<dyn UserPersistence>,
}

impl UserUseCases {
    pub fn new(hasher: Arc<dyn UserCredentialsHasher>, persistence: Arc<dyn UserPersistence>) -> Self {
        Self {
            hasher,
            persistence,
        }
    }

    #[instrument(skip(self))]
    pub async fn add(&self, username: &str, email: &str, password: &SecretString) -> AppResult<()> {
        info!("Adding user...");

        let hash = &self.hasher.hash_password(password.expose_secret())?;
        self.persistence.create_user(username, email, hash).await?;

        info!("Adding user finished.");

        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn list(&self) -> AppResult<Vec<UserListItem>> {
        info!("Listing users...");
        let users = self.persistence.list_users().await?;
        Ok(users)
    }

    #[instrument(skip(self))]
    pub async fn update(&self, id: Uuid, username: Option<&str>, email: Option<&str>, password: Option<&SecretString>) -> AppResult<()> {
        info!("Updating user...");

        let password_hash = match password {
            | Some(pw) => Some(self.hasher.hash_password(pw.expose_secret())?),
            | None => None,
        };

        self.persistence.update_user(id, username, email, password_hash.as_deref()).await?;

        info!("Updating user finished.");
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn delete(&self, id: Uuid) -> AppResult<()> {
        info!("Deleting user...");
        self.persistence.delete_user(id).await?;
        info!("Deleting user finished.");
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn get(&self, id: Uuid) -> AppResult<Option<UserDetail>> {
        info!("Getting user detail...");
        let res = self.persistence.get_user(id).await?;
        Ok(res)
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserListItem {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserDetail {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub created_at: NaiveDateTime,
}

#[cfg(test)]
mod test {
    use super::*;
    use async_trait::async_trait;
    use chrono::NaiveDateTime;
    use uuid::Uuid;

    struct MockUserPersistence;

    #[async_trait]
    impl UserPersistence for MockUserPersistence {
        async fn create_user(&self, username: &str, email: &str, _password_hash: &str) -> AppResult<()> {
            assert_eq!(username, "testuser");
            assert_eq!(email, "testuser@gmail.com");

            Ok(())
        }

        async fn list_users(&self) -> AppResult<Vec<UserListItem>> {
            Ok(vec![UserListItem {
                id: Uuid::nil(),
                username: "testuser".to_string(),
                email: "testuser@gmail.com".to_string(),
                created_at: NaiveDateTime::from_timestamp_opt(0, 0).unwrap(),
            }])
        }

        async fn get_user(&self, id: Uuid) -> AppResult<Option<UserDetail>> {
            if id.is_nil() {
                Ok(None)
            } else {
                Ok(Some(UserDetail {
                    id,
                    username: "testuser".into(),
                    email: "testuser@gmail.com".into(),
                    created_at: NaiveDateTime::from_timestamp_opt(0, 0).unwrap(),
                }))
            }
        }

        async fn update_user(&self, _id: Uuid, _username: Option<&str>, _email: Option<&str>, _password_hash: Option<&str>) -> AppResult<()> { Ok(()) }

        async fn delete_user(&self, _id: Uuid) -> AppResult<()> { Ok(()) }
    }

    struct MockUserCredentialsHasher;

    impl UserCredentialsHasher for MockUserCredentialsHasher {
        fn hash_password(&self, password: &str) -> AppResult<String> { Ok(format!("{}_hash", password)) }
    }

    #[tokio::test]
    async fn add_user_works() {
        let user_use_cases = UserUseCases::new(Arc::new(MockUserCredentialsHasher), Arc::new(MockUserPersistence));

        let result = user_use_cases.add("testuser", "testuser@gmail.com", &"testuser_pw".into()).await;

        assert!(result.is_ok());
    }
}
