use sqlx::{MySql,
           Pool};

/// 데이터베이스 초기화: 테이블 생성 및 샘플 데이터 삽입
pub async fn initialize_database(pool: &Pool<MySql>) -> Result<(), sqlx::Error> {
    log::info!("데이터베이스 초기화 시작...");

    // 테이블 생성
    create_tables(pool).await?;
    log::info!("테이블 생성 완료");

    // 샘플 데이터 삽입
    insert_sample_data(pool).await?;
    log::info!("샘플 데이터 삽입 완료");

    log::info!("데이터베이스 초기화 완료");
    Ok(())
}

/// members 테이블 생성
async fn create_tables(pool: &Pool<MySql>) -> Result<(), sqlx::Error> {
    // members 테이블이 존재하지 않으면 생성
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS members (
            id VARCHAR(8) PRIMARY KEY COMMENT '회원 ID (8자리)',
            name VARCHAR(64) NOT NULL COMMENT '회원 이름',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP COMMENT '생성 시각',
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '수정 시각'
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
        COMMENT='회원 정보 테이블'
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// 샘플 데이터 삽입
async fn insert_sample_data(pool: &Pool<MySql>) -> Result<(), sqlx::Error> {
    // 기존 데이터가 있는지 확인
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM members").fetch_one(pool).await?;

    // 데이터가 없을 때만 샘플 데이터 삽입
    if count.0 == 0 {
        log::info!("샘플 데이터 삽입 중...");

        let sample_members = vec![
            ("MEMB0001", "홍길동"),
            ("MEMB0002", "김철수"),
            ("MEMB0003", "이영희"),
            ("MEMB0004", "박민수"),
            ("MEMB0005", "정수진"),
        ];

        for (id, name) in sample_members {
            sqlx::query("INSERT INTO members (id, name) VALUES (?, ?)")
                .bind(id)
                .bind(name)
                .execute(pool)
                .await?;
        }

        log::info!("샘플 데이터 {} 건 삽입 완료", count.0);
    } else {
        log::info!("기존 데이터가 존재하여 샘플 데이터 삽입을 건너뜁니다. (현재 {} 건)", count.0);
    }

    Ok(())
}
