//! 워크플로 저장소 적합성 검사.
//!
//! **세 구현(SQLite · 인메모리 · PostgreSQL)이 같은 검사를 통과합니다.**

use super::BoxFut;
use crate::workflow::{self,
                      Run,
                      Status,
                      Store};
use serde_json::json;
use std::sync::Arc;

/// 빈 워크플로 저장소를 만듭니다.
pub type WorkflowFactory = Arc<dyn Fn() -> BoxFut<'static, Arc<dyn Store>> + Send + Sync>;

pub async fn workflow_store(make: WorkflowFactory) {
    missing_run_is_not_found(&make).await;
    save_and_load_round_trip(&make).await;
    save_is_an_upsert(&make).await;
    stored_state_is_isolated_from_the_caller(&make).await;
    stale_version_is_rejected(&make).await;
    concurrent_save_admits_exactly_one(&make).await;
    events_are_append_only_and_ordered(&make).await;
}

fn base_run() -> Run {
    Run {
        id: "refund-INV-1".into(),
        name: "refund".into(),
        status: Status::Running,
        completed: vec!["a".into(), "b".into()],
        values: json!({"amount": 1_200_000, "invoice": "INV-1"}).as_object().unwrap().clone(),
        started_at: Some(chrono::Utc::now()),
        updated_at: Some(chrono::Utc::now()),
        ..Default::default()
    }
}

async fn missing_run_is_not_found(make: &WorkflowFactory) {
    let s = make().await;
    assert!(s.load("nope").await.unwrap().is_none());
}

async fn save_and_load_round_trip(make: &WorkflowFactory) {
    let s = make().await;
    let saved = s.save(&base_run()).await.unwrap();
    assert_eq!(saved.version, 1, "첫 저장 뒤 버전은 1이어야 합니다");

    let got = s.load("refund-INV-1").await.unwrap().unwrap();
    assert_eq!(got.version, 1);
    assert_eq!(got.name, "refund");
    assert_eq!(got.status, Status::Running);
    assert_eq!(got.completed, vec!["a", "b"]);

    // JSON 저장소를 지나면 정수가 float 으로 올 수 있습니다 — 둘 다 받아들입니다.
    let amount = got.values.get("amount").unwrap();
    let n = amount.as_i64().or_else(|| amount.as_f64().map(|f| f as i64));
    assert_eq!(n, Some(1_200_000));
}

async fn save_is_an_upsert(make: &WorkflowFactory) {
    let s = make().await;
    let mut saved = s.save(&base_run()).await.unwrap();

    saved.status = Status::Completed;
    saved.completed = vec!["a".into()];
    let saved = s.save(&saved).await.unwrap();
    assert_eq!(saved.version, 2);

    let got = s.load("refund-INV-1").await.unwrap().unwrap();
    assert_eq!(got.status, Status::Completed);
    assert_eq!(got.completed.len(), 1);
    assert_eq!(got.version, 2);
}

/// 저장된 상태는 호출자의 나중 변경에 오염되지 않아야 합니다.
async fn stored_state_is_isolated_from_the_caller(make: &WorkflowFactory) {
    let s = make().await;
    let mut run = base_run();
    s.save(&run).await.unwrap();

    // 저장 **뒤에** 호출자가 자기 사본을 바꿉니다.
    run.completed.push("mutated".into());
    run.values.insert("amount".into(), json!(999));

    let got = s.load("refund-INV-1").await.unwrap().unwrap();
    assert_eq!(got.completed, vec!["a", "b"], "저장된 상태가 호출자의 나중 변경에 오염되었습니다");
    let amount = got.values.get("amount").unwrap();
    let n = amount.as_i64().or_else(|| amount.as_f64().map(|f| f as i64));
    assert_eq!(n, Some(1_200_000));
}

/// 오래된 버전으로 저장하면 거부됩니다 — 그러지 않으면 완료 단계가 **뒤로
/// 돌아갑니다.**
async fn stale_version_is_rejected(make: &WorkflowFactory) {
    let s = make().await;
    let base = s.save(&base_run()).await.unwrap(); // version 1

    // 두 인스턴스가 같은 스냅샷에서 갈라집니다.
    let mut a = base.clone();
    a.completed = vec!["a".into(), "b".into(), "c".into()];
    let mut b = base.clone();
    b.completed = vec!["a".into(), "x".into()];

    s.save(&a).await.unwrap(); // 먼저 저장 → version 2

    let err = s.save(&b).await.unwrap_err(); // 아직 version 1 을 들고 있음
    assert!(matches!(err, workflow::Error::VersionConflict), "오래된 버전 저장이 거부되지 않았습니다: {err}");

    // 진 쪽의 쓰기가 반영되지 않았어야 합니다.
    let got = s.load("refund-INV-1").await.unwrap().unwrap();
    assert_eq!(got.completed, vec!["a", "b", "c"]);
}

/// 동시 저장 중 **정확히 하나만** 성공합니다 (검사와 쓰기 사이에 틈이 없어야
/// 합니다).
async fn concurrent_save_admits_exactly_one(make: &WorkflowFactory) {
    let s = make().await;
    let base = s.save(&base_run()).await.unwrap();

    let barrier = Arc::new(tokio::sync::Barrier::new(8));
    let mut handles = Vec::new();
    for i in 0 .. 8 {
        let s = s.clone();
        let barrier = barrier.clone();
        let mut run = base.clone();
        run.completed.push(format!("step-{i}"));
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            s.save(&run).await
        }));
    }

    let mut ok = 0;
    for h in handles {
        if h.await.expect("task panicked").is_ok() {
            ok += 1;
        }
    }
    assert_eq!(ok, 1, "동시 저장 8개 중 {ok}개가 성공했습니다 — 정확히 1개여야 합니다");
}

async fn events_are_append_only_and_ordered(make: &WorkflowFactory) {
    let s = make().await;
    for (i, t) in ["workflow_started", "step_started", "step_completed"].iter().enumerate() {
        s.append_event(&workflow::Event {
            run_id: "r1".into(),
            at: chrono::Utc::now(),
            r#type: (*t).into(),
            step: "lookup".into(),
            attempt: i as i64,
            worker: "w1".into(),
            fencing_token: 1,
            message: String::new(),
        })
        .await
        .unwrap();
    }
    let got = s.events("r1").await.unwrap();
    assert_eq!(got.len(), 3);
    assert_eq!(got[0].r#type, "workflow_started");
    assert_eq!(got[2].r#type, "step_completed");
    // 다른 run 의 이벤트는 섞이지 않습니다.
    assert!(s.events("r2").await.unwrap().is_empty());
}
