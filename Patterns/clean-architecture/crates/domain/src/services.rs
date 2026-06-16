// domain/services.rs - 도메인 서비스

use crate::{models::User,
            repositories::UserRepository};

pub struct UserService<R: UserRepository> {
    repository: R,
}

impl<R: UserRepository> UserService<R> {
    pub fn delete_user(&self, id: &str) -> Result<(), String> { self.repository.delete(id) }

    pub fn new(repository: R) -> Self {
        Self {
            repository,
        }
    }

    pub fn get_user(&self, id: &str) -> Option<User> { self.repository.find_by_id(id) }

    pub fn create_user(&self, username: String, email: String) -> Result<User, String> {
        // 이메일 형식 검증 등 도메인 로직
        if !email.contains('@') {
            return Err("Invalid email format".to_string());
        }

        let user = User::new(username, email);
        self.repository.save(&user)?;
        Ok(user)
    }

    pub fn list_all_users(&self) -> Result<Vec<User>, String> { Ok(self.repository.find_all()) }

    pub fn deactivate_user(&self, id: &str) -> Result<User, String> {
        let mut user = self.repository.find_by_id(id).ok_or_else(|| format!("User with id {} not found", id))?;

        user.deactivate();
        self.repository.save(&user)?;
        Ok(user)
    }
}
