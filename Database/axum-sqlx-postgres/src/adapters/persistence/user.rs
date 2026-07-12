use crate::{adapters::persistence::PostgresPersistence,
            app_error::{AppError,
                        AppResult},
            entities::user::User,
            use_cases::user::{UserDetail,
                              UserListItem,
                              UserPersistence}};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde::Serialize;
use uuid::Uuid;

// User struct as stored in the db.
#[derive(sqlx::FromRow, Debug, Serialize)]
pub struct UserDb {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: NaiveDateTime,
}

impl From<UserDb> for User {
    fn from(user_db: UserDb) -> Self {
        User {
            id: user_db.id,
            username: user_db.username,
            password_hash: user_db.password_hash,
            created_at: user_db.created_at,
        }
    }
}

#[async_trait]
impl UserPersistence for PostgresPersistence {
    async fn create_user(&self, username: &str, email: &str, password_hash: &str) -> AppResult<()> {
        let uuid = Uuid::new_v4();

        sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, $4)")
            .bind(uuid)
            .bind(username)
            .bind(email)
            .bind(password_hash)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(())
    }

    async fn list_users(&self) -> AppResult<Vec<UserListItem>> {
        let rows = sqlx::query_as::<_, UserListItem>(r#"SELECT id, username, email, created_at FROM users ORDER BY created_at DESC"#)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(rows)
    }

    async fn get_user(&self, id: Uuid) -> AppResult<Option<UserDetail>> {
        let row = sqlx::query_as::<_, UserDetail>(r#"SELECT id, username, email, created_at FROM users WHERE id = $1"#)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(row)
    }

    async fn update_user(&self, id: Uuid, username: Option<&str>, email: Option<&str>, password_hash: Option<&str>) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET
                username = COALESCE($2, username),
                email = COALESCE($3, email),
                password_hash = COALESCE($4, password_hash)
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(())
    }

    async fn delete_user(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }
}
