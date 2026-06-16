// application/services.rs - 애플리케이션 서비스 (유스케이스)

use domain::{models::User,
             repositories::UserRepository,
             services::UserService};

pub struct UserApplicationService<R: UserRepository> {
    user_service: UserService<R>,
}

impl<R: UserRepository> UserApplicationService<R> {
    pub fn delete_user(&self, id: &str) -> Result<(), String> { self.user_service.delete_user(id) }

    pub fn new(repository: R) -> Self {
        Self {
            user_service: UserService::new(repository),
        }
    }

    pub fn register_user(&self, username: String, email: String) -> Result<UserDto, String> {
        // 비즈니스 규칙 적용
        if username.len() < 3 {
            return Err("Username must be at least 3 characters".to_string());
        }

        // 도메인 서비스 호출
        let user = self.user_service.create_user(username, email)?;

        // DTO로 변환하여 반환
        Ok(UserDto::from(user))
    }

    pub fn get_user_details(&self, id: &str) -> Option<UserDto> { self.user_service.get_user(id).map(UserDto::from) }

    pub fn deactivate_user(&self, id: &str) -> Result<UserDto, String> {
        let user = self.user_service.deactivate_user(id)?;
        Ok(UserDto::from(user))
    }

    pub fn list_all_users(&self) -> Result<Vec<UserDto>, String> {
        let users = self.user_service.list_all_users()?;
        Ok(users.into_iter().map(UserDto::from).collect())
    }
}

// DTO (Data Transfer Object)
use serde::{Deserialize,
            Serialize};
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserDto {
    pub id: Option<i64>,
    pub username: String,
    pub email: String,
    pub active: bool,
}

impl From<User> for UserDto {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            active: user.active,
        }
    }
}
