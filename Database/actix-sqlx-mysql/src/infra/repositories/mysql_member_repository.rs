use crate::domain::{Member,
                    MemberRepository};
use async_trait::async_trait;
use sqlx::{MySql,
           Pool,
           Row};

/// MySQL을 사용하는 MemberRepository 구현체
/// 도메인 계층의 MemberRepository 트레이트를 구현합니다.
pub struct MySqlMemberRepository {
    pool: Pool<MySql>,
}

impl MySqlMemberRepository {
    /// 새로운 MySqlMemberRepository 인스턴스를 생성합니다.
    pub fn new(pool: Pool<MySql>) -> Self {
        Self {
            pool,
        }
    }
}

#[async_trait]
impl MemberRepository for MySqlMemberRepository {
    async fn create(&self, name: String) -> Result<Member, Box<dyn std::error::Error>> {
        // MySQL 함수를 사용하여 시퀀스 ID 생성 및 Member 삽입
        let query = format!("INSERT INTO members(id, name) VALUES (fn_get_seq_8('MEMB'), '{}')", name);

        sqlx::query(&query).execute(&self.pool).await?;

        // 마지막으로 삽입된 ID 조회
        let row = sqlx::query("SELECT id, name FROM members ORDER BY id DESC LIMIT 1")
            .fetch_one(&self.pool)
            .await?;

        let id: String = row.try_get("id")?;
        let name: String = row.try_get("name")?;

        Ok(Member::new(id, name))
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Member>, Box<dyn std::error::Error>> {
        let result = sqlx::query("SELECT id, name FROM members WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        match result {
            | Some(row) => {
                let id: String = row.try_get("id")?;
                let name: String = row.try_get("name")?;
                Ok(Some(Member::new(id, name)))
            },
            | None => Ok(None),
        }
    }

    async fn find_all(&self) -> Result<Vec<Member>, Box<dyn std::error::Error>> {
        let rows = sqlx::query("SELECT id, name FROM members").fetch_all(&self.pool).await?;

        let members = rows
            .iter()
            .map(|row| {
                let id: String = row.try_get("id").unwrap();
                let name: String = row.try_get("name").unwrap();
                Member::new(id, name)
            })
            .collect();

        Ok(members)
    }

    async fn count(&self) -> Result<i64, Box<dyn std::error::Error>> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM members").fetch_one(&self.pool).await?;

        let count: i64 = row.try_get("count")?;
        Ok(count)
    }

    async fn update(&self, member: &Member) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query("UPDATE members SET name = ? WHERE id = ?")
            .bind(&member.name)
            .bind(&member.id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query("DELETE FROM members WHERE id = ?").bind(id).execute(&self.pool).await?;

        Ok(())
    }
}
