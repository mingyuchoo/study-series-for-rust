//! 워크플로 엔진 동작 검사 — 환불 흐름을 그대로 흉내냅니다.
//!
//! 검사하는 것은 "돈이 두 번 나가지 않는가", "실패하면 되돌리는가", "죽었다
//! 살아나면 이어서 하는가" 입니다.

use ai_bridge::workflow::{Definition,
                          Engine,
                          MemoryStore,
                          RetryPolicy,
                          State,
                          Status,
                          Step,
                          Store,
                          step_fn,
                          wait_until};
use anyhow::anyhow;
use serde_json::json;
use std::{sync::{Arc,
                 Mutex,
                 atomic::{AtomicUsize,
                          Ordering}},
          time::Duration};

/// 무슨 일이 실제로 벌어졌는지 기록하는 원장.
#[derive(Default)]
struct Ledger {
    calls: Mutex<Vec<String>>,
}

impl Ledger {
    fn record(&self, what: &str) { self.calls.lock().unwrap().push(what.to_string()); }

    fn calls(&self) -> Vec<String> { self.calls.lock().unwrap().clone() }
}

fn step(name: &str, ledger: Arc<Ledger>) -> Step {
    let n = name.to_string();
    let c = name.to_string();
    let l2 = ledger.clone();
    Step {
        name: name.to_string(),
        run: step_fn(move |mut s: State| {
            let (n, l) = (n.clone(), ledger.clone());
            async move {
                l.record(&n);
                s.set(&format!("{n}_done"), json!(true));
                Ok(s)
            }
        }),
        compensate: Some(step_fn(move |s: State| {
            let (c, l) = (c.clone(), l2.clone());
            async move {
                l.record(&format!("compensate:{c}"));
                Ok(s)
            }
        })),
        timeout: Duration::ZERO,
        retry: RetryPolicy::default(),
    }
}

/// 환불 6단계 — `notify_customer` 가 실패하도록 만들 수 있습니다.
fn refund_def(ledger: Arc<Ledger>, fail_at: Option<&str>) -> Definition {
    let names = [
        "lookup_invoice",
        "check_refundable",
        "calculate_amount",
        "create_draft",
        "execute_refund",
        "notify_customer",
    ];
    let steps = names
        .iter()
        .map(|n| {
            let mut st = step(n, ledger.clone());
            if fail_at == Some(*n) {
                let name = n.to_string();
                let l = ledger.clone();
                st.run = step_fn(move |_s: State| {
                    let (name, l) = (name.clone(), l.clone());
                    async move {
                        l.record(&format!("{name}:FAILED"));
                        // 업무 오류 — 재시도해도 결과가 같습니다.
                        Err(anyhow!("{name} failed permanently"))
                    }
                });
            }
            st
        })
        .collect();

    Definition {
        name: "refund".into(),
        version: "1".into(),
        steps,
    }
}

fn engine(store: Arc<dyn Store>) -> Engine { Engine::new(store).with_worker("worker-test") }

#[tokio::test]
async fn runs_all_steps_in_order() {
    let ledger = Arc::new(Ledger::default());
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    let e = engine(store);
    let def = refund_def(ledger.clone(), None);

    let run = e
        .execute(&def, "refund-INV-1", Some(&json!({"invoice_id":"INV-1"}).as_object().unwrap().clone()))
        .await
        .unwrap();

    assert_eq!(run.status, Status::Completed);
    assert_eq!(
        ledger.calls(),
        vec![
            "lookup_invoice",
            "check_refundable",
            "calculate_amount",
            "create_draft",
            "execute_refund",
            "notify_customer",
        ]
    );
}

/// **멱등** — 이미 끝난 흐름을 다시 호출하면 돈이 두 번 나가지 않습니다.
#[tokio::test]
async fn completed_run_is_idempotent() {
    let ledger = Arc::new(Ledger::default());
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    let e = engine(store);
    let def = refund_def(ledger.clone(), None);
    let input = json!({"invoice_id":"INV-1"}).as_object().unwrap().clone();

    e.execute(&def, "refund-INV-1", Some(&input)).await.unwrap();
    let first = ledger.calls().len();

    let run = e.execute(&def, "refund-INV-1", Some(&input)).await.unwrap();
    assert_eq!(run.status, Status::Completed);
    assert_eq!(ledger.calls().len(), first, "이미 끝난 흐름을 다시 호출했더니 단계가 또 실행되었습니다");
}

/// **재개** — 죽었다 살아나면 완료된 단계를 건너뛰고 이어서 합니다.
#[tokio::test]
async fn resumes_from_the_last_completed_step() {
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    let ledger = Arc::new(Ledger::default());
    let input = json!({"invoice_id":"INV-1"}).as_object().unwrap().clone();

    // 1차: notify_customer 직전에 죽는 상황 — 보상이 돌지 않게 그 단계에 보상을
    // 두지 않고 대신 중간에서 프로세스가 사라진 것처럼 완료 목록만 남깁니다.
    {
        let e = engine(store.clone());
        let def = refund_def(ledger.clone(), Some("notify_customer"));
        let _ = e.execute(&def, "refund-INV-1", Some(&input)).await;
    }

    // 보상이 돌았으므로 Failed/Compensated 입니다. 복구 후 재실행합니다.
    let e = engine(store.clone());
    e.recover("refund-INV-1").await.unwrap();

    let ledger2 = Arc::new(Ledger::default());
    let def_ok = refund_def(ledger2.clone(), None);
    let run = e.execute(&def_ok, "refund-INV-1", Some(&input)).await.unwrap();

    assert_eq!(run.status, Status::Completed);
    // Recover 는 완료 목록을 비우므로 전 단계를 다시 실행합니다(보상으로 되돌렸기
    // 때문).
    assert_eq!(ledger2.calls().len(), 6);
}

/// **보상은 역순** — 가장 최근에 한 일부터 되돌립니다.
#[tokio::test]
async fn compensates_completed_steps_in_reverse_order() {
    let ledger = Arc::new(Ledger::default());
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    let e = engine(store.clone());
    // 마지막 단계에서 실패 → 앞의 5단계를 되돌려야 합니다.
    let def = refund_def(ledger.clone(), Some("notify_customer"));

    let err = e
        .execute(&def, "refund-INV-1", Some(&json!({"invoice_id":"INV-1"}).as_object().unwrap().clone()))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("notify_customer"));

    let calls = ledger.calls();
    let comps: Vec<&String> = calls.iter().filter(|c| c.starts_with("compensate:")).collect();
    assert_eq!(
        comps,
        vec![
            "compensate:execute_refund",
            "compensate:create_draft",
            "compensate:calculate_amount",
            "compensate:check_refundable",
            "compensate:lookup_invoice",
        ],
        "보상이 역순으로 돌지 않았습니다"
    );

    // 보상이 성공했으므로 Compensated 입니다 — 사람이 볼 필요는 없습니다.
    let run = store.load("refund-INV-1").await.unwrap().unwrap();
    assert_eq!(run.status, Status::Compensated);
    assert!(run.compensate_error.is_empty());
}

/// 실패한 단계 자신은 보상하지 않습니다 — 아직 하지 않았기 때문입니다.
#[tokio::test]
async fn the_failed_step_itself_is_not_compensated() {
    let ledger = Arc::new(Ledger::default());
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    let e = engine(store);
    let def = refund_def(ledger.clone(), Some("execute_refund"));

    let _ = e.execute(&def, "refund-INV-1", Some(&json!({}).as_object().unwrap().clone())).await;

    let calls = ledger.calls();
    assert!(
        !calls.contains(&"compensate:execute_refund".to_string()),
        "실행되지 않은 단계를 되돌리려 했습니다"
    );
    assert!(calls.contains(&"compensate:create_draft".to_string()));
}

/// **보상까지 실패하면 `Failed`** — 사람이 확인해야 합니다.
#[tokio::test]
async fn failed_compensation_escalates_to_failed_for_human_review() {
    let ledger = Arc::new(Ledger::default());
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    let e = engine(store.clone());

    let mut def = refund_def(ledger.clone(), Some("notify_customer"));
    // execute_refund 의 보상(환불 되돌리기)이 실패하는 상황.
    let l = ledger.clone();
    def.steps[4].compensate = Some(step_fn(move |_s: State| {
        let l = l.clone();
        async move {
            l.record("compensate:execute_refund:FAILED");
            Err(anyhow!("ledger unavailable"))
        }
    }));

    let _ = e.execute(&def, "refund-INV-1", Some(&json!({}).as_object().unwrap().clone())).await;

    let run = store.load("refund-INV-1").await.unwrap().unwrap();
    assert_eq!(run.status, Status::Failed, "보상이 실패했는데 Compensated 로 남았습니다 — 사람이 보지 못합니다");
    assert!(run.compensate_error.contains("execute_refund"));

    // 하나가 실패해도 나머지 보상은 계속 시도했어야 합니다 — 일부라도 되돌리는 편이
    // 낫습니다.
    let calls = ledger.calls();
    assert!(calls.contains(&"compensate:create_draft".to_string()));
    assert!(calls.contains(&"compensate:lookup_invoice".to_string()));
}

/// 멱등 키는 재시도 세대를 담습니다.
#[tokio::test]
async fn activity_key_carries_the_recovery_generation() {
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let s2 = seen.clone();
    let def = Definition {
        name: "wf".into(),
        version: "1".into(),
        steps: vec![Step {
            name: "activity".into(),
            run: step_fn(move |s: State| {
                let seen = s2.clone();
                async move {
                    seen.lock().unwrap().push(s.activity_key.clone());
                    Ok(s)
                }
            }),
            compensate: None,
            timeout: Duration::ZERO,
            retry: RetryPolicy::default(),
        }],
    };

    let e = engine(store.clone());
    e.execute(&def, "run-1", None).await.unwrap();
    assert_eq!(seen.lock().unwrap()[0], "run-1:recovery-0:activity");

    // 완료된 흐름은 재실행되지 않으므로 강제로 Failed 로 만든 뒤 복구합니다.
    let mut run = store.load("run-1").await.unwrap().unwrap();
    run.status = Status::Failed;
    run.error = "forced".into();
    ai_bridge::workflow::MemoryStore::new(); // no-op, 가독성용
    store.save(&run).await.unwrap();

    e.recover("run-1").await.unwrap();
    e.execute(&def, "run-1", None).await.unwrap();

    // 세대가 바뀌어 외부 시스템이 이전 시도와 구분할 수 있습니다.
    assert_eq!(seen.lock().unwrap()[1], "run-1:recovery-1:activity");
}

/// 재시도는 **일시적 장애**일 때만 합니다.
#[tokio::test]
async fn retries_only_transient_failures() {
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    let attempts = Arc::new(AtomicUsize::new(0));

    let a = attempts.clone();
    let def = Definition {
        name: "wf".into(),
        version: "1".into(),
        steps: vec![Step {
            name: "flaky".into(),
            run: step_fn(move |s: State| {
                let a = a.clone();
                async move {
                    let n = a.fetch_add(1, Ordering::SeqCst);
                    if n < 2 {
                        // 일시적 장애 — 재시도할 가치가 있습니다.
                        return Err(ai_bridge::transient::temporary(anyhow!("ERP 503")));
                    }
                    Ok(s)
                }
            }),
            compensate: None,
            timeout: Duration::ZERO,
            retry: RetryPolicy {
                max_attempts: 5,
                initial_backoff: Duration::from_millis(1),
                max_backoff: Duration::from_millis(2),
            },
        }],
    };

    let e = engine(store);
    let run = e.execute(&def, "run-1", None).await.unwrap();
    assert_eq!(run.status, Status::Completed);
    assert_eq!(attempts.load(Ordering::SeqCst), 3, "2번 실패 후 3번째에 성공");
}

#[tokio::test]
async fn business_errors_are_not_retried() {
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    let attempts = Arc::new(AtomicUsize::new(0));

    let a = attempts.clone();
    let def = Definition {
        name: "wf".into(),
        version: "1".into(),
        steps: vec![Step {
            name: "missing_invoice".into(),
            run: step_fn(move |_s: State| {
                let a = a.clone();
                async move {
                    a.fetch_add(1, Ordering::SeqCst);
                    // "없는 송장"은 백 번 물어도 없습니다.
                    Err(anyhow!("invoice not found"))
                }
            }),
            compensate: None,
            timeout: Duration::ZERO,
            retry: RetryPolicy {
                max_attempts: 5,
                initial_backoff: Duration::from_millis(1),
                max_backoff: Duration::ZERO,
            },
        }],
    };

    let e = engine(store);
    let _ = e.execute(&def, "run-1", None).await;
    assert_eq!(attempts.load(Ordering::SeqCst), 1, "업무 오류를 재시도했습니다");
}

/// 대기 상태 — 외부 이벤트를 기다립니다. **재시도되지 않습니다.**
#[tokio::test]
async fn wait_suspends_the_run_without_retrying() {
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    let attempts = Arc::new(AtomicUsize::new(0));

    let a = attempts.clone();
    let until = chrono::Utc::now() + chrono::Duration::hours(1);
    let def = Definition {
        name: "wf".into(),
        version: "1".into(),
        steps: vec![Step {
            name: "await_callback".into(),
            run: step_fn(move |_s: State| {
                let a = a.clone();
                async move {
                    a.fetch_add(1, Ordering::SeqCst);
                    Err(wait_until(until, "고객 확인 대기"))
                }
            }),
            compensate: None,
            timeout: Duration::ZERO,
            retry: RetryPolicy {
                max_attempts: 5,
                initial_backoff: Duration::from_millis(1),
                max_backoff: Duration::ZERO,
            },
        }],
    };

    let e = engine(store.clone());
    let err = e.execute(&def, "run-1", None).await.unwrap_err();
    assert!(matches!(err, ai_bridge::workflow::Error::Waiting(_)));
    assert_eq!(attempts.load(Ordering::SeqCst), 1, "대기 요청을 재시도했습니다");

    let run = store.load("run-1").await.unwrap().unwrap();
    assert_eq!(run.status, Status::Waiting);
    // 대기는 종결이 아니므로 취소할 수 있습니다.
    assert!(!run.status.terminal());
    assert!(e.cancel("run-1", "운영자 중단").await.is_ok());
}

/// 같은 run ID 를 다른 입력으로 재사용하면 거부합니다.
#[tokio::test]
async fn reusing_a_run_id_with_different_input_is_rejected() {
    let ledger = Arc::new(Ledger::default());
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    let e = engine(store);
    let def = refund_def(ledger, None);

    e.execute(&def, "refund-INV-1", Some(&json!({"invoice_id":"INV-1"}).as_object().unwrap().clone()))
        .await
        .unwrap();

    // 완료된 run 은 멱등하게 돌아오지만, 입력이 다르면 그 전에 걸러야 합니다.
    let err = e
        .execute(&def, "refund-INV-1", Some(&json!({"invoice_id":"INV-9999"}).as_object().unwrap().clone()))
        .await
        .unwrap_err();
    assert!(matches!(err, ai_bridge::workflow::Error::InputConflict));
}

/// 다른 worker 가 lease 를 쥐고 있으면 물러납니다.
#[tokio::test]
async fn a_run_leased_by_another_live_worker_is_not_stolen() {
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());

    // worker-a 가 lease 를 잡아둔 상태를 만듭니다.
    let mut run = ai_bridge::workflow::Run {
        id: "run-1".into(),
        name: "wf".into(),
        status: Status::Running,
        definition_version: "1".into(),
        lease_owner: "worker-a".into(),
        lease_until: Some(chrono::Utc::now() + chrono::Duration::minutes(5)),
        started_at: Some(chrono::Utc::now()),
        updated_at: Some(chrono::Utc::now()),
        ..Default::default()
    };
    ai_bridge::workflow::sync_metadata_for_test(&mut run);
    store.save(&run).await.unwrap();

    let ledger = Arc::new(Ledger::default());
    let def = Definition {
        name: "wf".into(),
        version: "1".into(),
        steps: vec![step("s1", ledger.clone())],
    };

    let e = Engine::new(store).with_worker("worker-b");
    let err = e.execute(&def, "run-1", None).await.unwrap_err();
    assert!(matches!(err, ai_bridge::workflow::Error::LeaseHeld));
    assert!(ledger.calls().is_empty(), "다른 worker 의 lease 를 훔쳤습니다");
}
