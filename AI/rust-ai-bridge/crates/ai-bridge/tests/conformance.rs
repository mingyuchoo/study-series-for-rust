//! 저장소 적합성 검사 — 모든 구현이 **같은 검사**를 통과해야 합니다.
//!
//! PostgreSQL 검사는 `TEST_POSTGRES_DSN` 이 있을 때만 돌고, 없으면 건너뜁니다 —
//! CI 에 백엔드가 없어도 나머지는 통과합니다.

use ai_bridge::{approval::{self,
                           PostgresApprovalStore,
                           SqliteApprovalStore},
                clock::SharedClock,
                storetest};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn sqlite_approval_store_is_conformant() {
    // **파일 기반 + 다중 연결**이어야 동시성 불변식이 실제로 검사됩니다. 인메모리
    // DB 는 연결마다 별개라 연결을 하나로 묶어야 하고, 그러면 동시 소비 검사가
    // 무의미해집니다.
    let dir = tempfile::tempdir().unwrap();
    let dir = Arc::new(dir);
    let counter = std::sync::atomic::AtomicU32::new(0);

    let make: storetest::ApprovalFactory = Arc::new(move |clock: SharedClock| {
        let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let path = dir.path().join(format!("approval-{n}.db"));
        Box::pin(async move {
            let mut s = SqliteApprovalStore::open(&path).await.unwrap();
            s.set_clock(clock);
            Arc::new(s) as Arc<dyn approval::Store>
        })
    });
    storetest::approval_store(make).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_approval_store_is_conformant() {
    let Ok(dsn) = std::env::var("TEST_POSTGRES_DSN") else {
        eprintln!("TEST_POSTGRES_DSN 이 없어 PostgreSQL 적합성 검사를 건너뜁니다");
        return;
    };

    // 검사마다 격리된 스키마를 씁니다.
    let make: storetest::ApprovalFactory = Arc::new(move |clock: SharedClock| {
        let dsn = dsn.clone();
        Box::pin(async move {
            let pool = sqlx::postgres::PgPoolOptions::new().max_connections(10).connect(&dsn).await.unwrap();
            sqlx::query("DROP TABLE IF EXISTS approval_request").execute(&pool).await.unwrap();
            let mut s = PostgresApprovalStore::from_pool(pool).await.unwrap();
            s.set_clock(clock);
            Arc::new(s) as Arc<dyn approval::Store>
        })
    });
    storetest::approval_store(make).await;
}

// --- 워크플로: 세 구현이 같은 검사를 통과합니다 ---

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn memory_workflow_store_is_conformant() {
    let make: storetest::WorkflowFactory =
        Arc::new(|| Box::pin(async { Arc::new(ai_bridge::workflow::MemoryStore::new()) as Arc<dyn ai_bridge::workflow::Store> }));
    storetest::workflow_store(make).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn sqlite_workflow_store_is_conformant() {
    let dir = Arc::new(tempfile::tempdir().unwrap());
    let counter = std::sync::atomic::AtomicU32::new(0);
    let make: storetest::WorkflowFactory = Arc::new(move || {
        let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let path = dir.path().join(format!("wf-{n}.db"));
        Box::pin(async move { Arc::new(ai_bridge::workflow::SqliteWorkflowStore::open(&path).await.unwrap()) as Arc<dyn ai_bridge::workflow::Store> })
    });
    storetest::workflow_store(make).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_workflow_store_is_conformant() {
    let Ok(dsn) = std::env::var("TEST_POSTGRES_DSN") else {
        eprintln!("TEST_POSTGRES_DSN 이 없어 PostgreSQL 적합성 검사를 건너뜁니다");
        return;
    };
    let make: storetest::WorkflowFactory = Arc::new(move || {
        let dsn = dsn.clone();
        Box::pin(async move {
            let pool = sqlx::postgres::PgPoolOptions::new().max_connections(10).connect(&dsn).await.unwrap();
            sqlx::query("DROP TABLE IF EXISTS workflow_run").execute(&pool).await.unwrap();
            sqlx::query("DROP TABLE IF EXISTS workflow_event").execute(&pool).await.unwrap();
            Arc::new(ai_bridge::workflow::PostgresWorkflowStore::from_pool(pool).await.unwrap()) as Arc<dyn ai_bridge::workflow::Store>
        })
    });
    storetest::workflow_store(make).await;
}

// --- PostgreSQL 감사 로거: 해시 체인 + 변조 탐지 + 보존 ---
//
// PostgreSQL 판은 여러 인스턴스가 동시에 쓰므로 advisory lock 으로 체인 꼬리를
// 직렬화합니다. SQLite 판과 다른 이 경로를 검사합니다.
#[tokio::test]
async fn postgres_audit_logger_hash_chain_and_retention() {
    use ai_bridge::audit::{Discard,
                           Entry,
                           Policy,
                           PostgresLogger,
                           Purger,
                           Reader,
                           Recorder};
    use chrono::TimeZone as _;

    let Ok(dsn) = std::env::var("TEST_POSTGRES_DSN") else {
        eprintln!("TEST_POSTGRES_DSN 이 없어 PostgreSQL 감사 검사를 건너뜁니다");
        return;
    };

    let pool = sqlx::postgres::PgPoolOptions::new().max_connections(10).connect(&dsn).await.unwrap();
    // 격리: 매번 깨끗한 테이블에서 시작합니다.
    sqlx::query("DROP TABLE IF EXISTS audit_integrity").execute(&pool).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS audit_log").execute(&pool).await.unwrap();
    let logger = PostgresLogger::from_pool(pool.clone()).await.unwrap();

    let epoch = chrono::Utc.with_ymd_and_hms(2026, 7, 10, 9, 0, 0).unwrap();
    let entry = move |i: i64| Entry {
        timestamp: epoch + chrono::Duration::seconds(i),
        actor: "emp-sales-01".into(),
        tool: "get_invoice_status".into(),
        system: "erp".into(),
        access: "read".into(),
        decision: "allowed".into(),
        ..Default::default()
    };

    // 여러 인스턴스를 흉내내어 동시에 씁니다 — advisory lock 이 체인을 직렬화해야
    // 합니다.
    let mut handles = Vec::new();
    for i in 0 .. 10 {
        let l = logger.clone();
        let e = entry(i); // 스폰 전에 만들어 넘깁니다.
        handles.push(tokio::spawn(async move { l.log(&e).await }));
    }
    for h in handles {
        h.await.unwrap().unwrap();
    }

    assert_eq!(logger.recent(100).await.unwrap().len(), 10);
    // 동시 기록에도 해시 체인이 온전해야 합니다.
    logger.verify_integrity().await.expect("동시 기록 후 체인이 깨졌습니다");

    // 변조를 탐지해야 합니다.
    sqlx::query("UPDATE audit_log SET actor = 'attacker' WHERE id = (SELECT MIN(id) FROM audit_log)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(logger.verify_integrity().await.is_err(), "행 변조가 탐지되지 않았습니다");

    // 보존: 오래된 기록은 아카이브 후 삭제, 최근 기록은 유지.
    sqlx::query("DELETE FROM audit_log").execute(&pool).await.unwrap();
    logger.log(&entry(0)).await.unwrap(); // 최근
    let mut old = entry(0);
    old.timestamp = epoch - chrono::Duration::days(100);
    logger.log(&old).await.unwrap();

    let pol = Policy {
        by_tool: std::collections::HashMap::from([("get_invoice_status".to_string(), 30)]),
        default: 0,
    };
    let purged = logger.purge(&pol, epoch, &Discard).await.unwrap();
    assert_eq!(purged.deleted, 1, "보존 기간 초과 기록만 지워져야 합니다");
    assert_eq!(logger.recent(100).await.unwrap().len(), 1);
}
