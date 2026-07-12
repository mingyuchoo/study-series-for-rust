use sqlx::{MySql,
           Pool};

/// MySQL 데이터베이스 연결 풀을 생성합니다.
pub async fn create_pool(database_url: &str) -> Result<Pool<MySql>, sqlx::Error> { sqlx::MySqlPool::connect(database_url).await }

/// 환경 변수에서 데이터베이스 URL을 가져오거나 기본값을 사용합니다.
pub fn get_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        // 도커 컴포즈 기본값: 사용자 test / 비밀번호 test / 데이터베이스 test
        "mysql://test:test@localhost:3306/test".to_string()
    })
}
