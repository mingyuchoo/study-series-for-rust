//! 저장소 인터페이스 **적합성 테스트**.
//!
//! 인터페이스를 갈라두는 것만으로 교체가 가능해지지는 않습니다. 시그니처만 맞춘
//! 구현은 컴파일되지만 승인 관문을 무너뜨릴 수 있습니다.
//!
//! 그래서 검사하는 것은 구현이 아니라 **의미**입니다. 새 구현은 생성자 하나만
//! 넘기면 같은 검사를 통과해야 합니다. **여러 구현이 같은 검사를 통과한다는
//! 것이 "교체 가능하다"의 뜻입니다.**

mod workflow;
use crate::{approval::{self,
                       Status},
            clock::{SharedClock,
                    TestClock}};
use chrono::{TimeZone,
             Utc};
use serde_json::{Map,
                 Value,
                 json};
use std::{future::Future,
          pin::Pin,
          sync::Arc,
          time::Duration};
pub use workflow::{WorkflowFactory,
                   workflow_store};

pub(crate) type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// 시계를 받아 **빈** 승인 저장소를 만듭니다. 검사마다 새로 호출됩니다.
pub type ApprovalFactory = Arc<dyn Fn(SharedClock) -> BoxFut<'static, Arc<dyn approval::Store>> + Send + Sync>;

fn args(v: Value) -> Map<String, Value> { v.as_object().cloned().unwrap_or_default() }

/// 적합성 스위트의 기준 시각 (2026-07-10T09:00:00Z).
fn epoch() -> chrono::DateTime<Utc> { Utc.with_ymd_and_hms(2026, 7, 10, 9, 0, 0).unwrap() }

fn new_clock() -> Arc<TestClock> { Arc::new(TestClock::new(epoch())) }

/// 승인 저장소가 지켜야 할 모든 것.
pub async fn approval_store(make: ApprovalFactory) {
    approval_is_single_use(&make).await;
    concurrent_consumption_admits_exactly_one(&make).await;
    fingerprint_binds_to_args(&make).await;
    requester_cannot_decide_their_own_request(&make).await;
    approver_is_required(&make).await;
    expired_approval_does_not_authorize_execution(&make).await;
    expiry_boundary_is_exclusive(&make).await;
    default_ttl_is_applied(&make).await;
    rejected_requests_never_expire(&make).await;
    a_decided_request_cannot_be_decided_again(&make).await;
    missing_request_is_not_found(&make).await;
    list_filters_by_status(&make).await;
}

/// 승인은 단회성입니다 — 한 번 소비되면 같은 호출이라도 다시 승인받아야 합니다.
async fn approval_is_single_use(make: &ApprovalFactory) {
    let clock = new_clock();
    let s = make(clock.clone()).await;
    let a = args(json!({"invoice_id": "INV-1"}));

    let req = s.ensure("emp-1", "process_refund", &a, Duration::ZERO).await.unwrap();
    assert_eq!(req.status, Status::Pending, "첫 요청은 대기 상태여야 합니다");

    s.decide(&req.id, true, "manager-01", "").await.unwrap();

    // 승인 뒤 첫 호출은 실행 허가를 받습니다.
    let consumed = s.ensure("emp-1", "process_refund", &a, Duration::ZERO).await.unwrap();
    assert_eq!(consumed.status, Status::Approved);

    // **두 번째 호출은 다시 승인을 받아야 합니다.**
    let again = s.ensure("emp-1", "process_refund", &a, Duration::ZERO).await.unwrap();
    assert_eq!(again.status, Status::Pending, "소비된 승인이 재사용되었습니다 — 도구가 두 번 실행됩니다");
    assert_ne!(again.id, req.id);
}

/// 동시 소비 중 **정확히 하나만** 승인되고, **나머지는 오류 없이 새 대기
/// 요청**을 받습니다.
///
/// 두 번째 단언이 없으면 이 검사는 이빨이 없습니다. SQLite 의 WAL 스냅샷 격리는
/// 잠금 전략이 틀려도 이중 소비만큼은 막아 주기 때문입니다 — 대신 진 쪽에게
/// `SQLITE_BUSY` 를 던집니다. 즉 잘못된 구현은 **안전성이 아니라 가용성**을
/// 잃습니다: 정당한 호출자 여럿이 대기 요청 대신 오류를 받습니다. 그래서 둘 다
/// 검사합니다.
async fn concurrent_consumption_admits_exactly_one(make: &ApprovalFactory) {
    let clock = new_clock();
    let s = make(clock.clone()).await;
    let a = args(json!({"invoice_id": "INV-1"}));

    let req = s.ensure("emp-1", "process_refund", &a, Duration::ZERO).await.unwrap();
    s.decide(&req.id, true, "manager-01", "").await.unwrap();

    // 모두 같은 순간에 출발시켜 실제로 경합하게 만듭니다.
    let barrier = Arc::new(tokio::sync::Barrier::new(8));
    let mut handles = Vec::new();
    for _ in 0 .. 8 {
        let s = s.clone();
        let a = a.clone();
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            s.ensure("emp-1", "process_refund", &a, Duration::ZERO).await
        }));
    }

    let (mut approved, mut pending, mut errors) = (0, 0, 0);
    for h in handles {
        match h.await.expect("task panicked") {
            | Ok(r) if r.status == Status::Approved => approved += 1,
            | Ok(r) if r.status == Status::Pending => pending += 1,
            | Ok(_) => {},
            | Err(_) => errors += 1,
        }
    }

    // 안전성: 승인은 단 한 번만 소비됩니다.
    assert_eq!(approved, 1, "동시 호출 8개 중 {approved}개가 승인을 소비했습니다 — 도구가 두 번 실행됩니다");
    // 가용성: 진 쪽은 오류가 아니라 새 대기 요청을 받아야 합니다.
    assert_eq!(
        errors, 0,
        "동시 호출 중 {errors}개가 오류를 받았습니다 — 경합에서 진 호출자는 \
         오류가 아니라 새 대기 요청을 받아야 합니다(잠금 전략을 확인하세요)"
    );
    assert_eq!(pending, 7, "나머지 7개는 새 대기 요청이어야 합니다");
}

/// 지문은 인자에 묶입니다 — 인자가 다르면 기존 승인이 적용되지 않습니다.
async fn fingerprint_binds_to_args(make: &ApprovalFactory) {
    let clock = new_clock();
    let s = make(clock.clone()).await;

    let req = s
        .ensure("emp-1", "submit_purchase_request", &args(json!({"amount": 100})), Duration::ZERO)
        .await
        .unwrap();
    s.decide(&req.id, true, "manager-01", "").await.unwrap();

    // 금액을 바꿔 호출하면 **새 승인 요청**이 되어야 합니다.
    let other = s
        .ensure("emp-1", "submit_purchase_request", &args(json!({"amount": 999})), Duration::ZERO)
        .await
        .unwrap();
    assert_eq!(other.status, Status::Pending, "인자가 다른데 기존 승인이 적용되었습니다");
}

/// **요청자는 자기 요청을 결정할 수 없습니다.** 저장소가 막아야 합니다 — CLI 가
/// 아니라.
async fn requester_cannot_decide_their_own_request(make: &ApprovalFactory) {
    let clock = new_clock();
    let s = make(clock.clone()).await;
    let req = s.ensure("emp-1", "create_support_ticket", &args(json!({})), Duration::ZERO).await.unwrap();

    // 승인도 거부도 막습니다 — 요청자가 거부·재요청을 반복해 이력을 흐릴 수
    // 있습니다.
    let err = s.decide(&req.id, true, "emp-1", "").await.unwrap_err();
    assert!(matches!(err, approval::Error::SelfApproval(_)));
    let err = s.decide(&req.id, false, "emp-1", "").await.unwrap_err();
    assert!(matches!(err, approval::Error::SelfApproval(_)));

    // 실패한 시도가 상태를 바꾸지 않았어야 합니다.
    assert_eq!(s.get(&req.id).await.unwrap().status, Status::Pending);

    // 다른 사람은 결정할 수 있습니다.
    let decided = s.decide(&req.id, true, "manager-01", "").await.unwrap();
    assert_eq!(decided.status, Status::Approved);
}

/// 결정자 없는 승인은 감사 추적이 불가능합니다.
async fn approver_is_required(make: &ApprovalFactory) {
    let clock = new_clock();
    let s = make(clock.clone()).await;
    let req = s.ensure("emp-1", "create_support_ticket", &args(json!({})), Duration::ZERO).await.unwrap();
    let err = s.decide(&req.id, true, "", "").await.unwrap_err();
    assert!(matches!(err, approval::Error::NoApprover));
}

/// 만료된 승인으로는 실행할 수 없고, **새 대기 요청**이 만들어집니다.
///
/// 그리고 **TTL 시계는 결정 시점부터 흐릅니다** — 요청 시점이 아닙니다.
async fn expired_approval_does_not_authorize_execution(make: &ApprovalFactory) {
    let clock = new_clock();
    let s = make(clock.clone()).await;
    let a = args(json!({"invoice_id": "INV-1"}));
    let ttl = Duration::from_secs(15 * 60);

    let req = s.ensure("emp-1", "process_refund", &a, ttl).await.unwrap();

    // 관리자가 3시간 뒤에 결정합니다.
    clock.advance(chrono::Duration::hours(3));
    let decided = s.decide(&req.id, true, "manager-01", "").await.unwrap();

    // 만료 시각은 **요청 시점 + TTL** 이 아니라 **결정 시점 + TTL** 이어야 합니다.
    let want = epoch() + chrono::Duration::hours(3) + chrono::Duration::minutes(15);
    assert_eq!(decided.expires_at.unwrap(), want, "TTL 시계가 결정 시점이 아니라 요청 시점부터 흘렀습니다");

    // 유효 기간이 지난 뒤 호출하면 실행되지 않고 새 요청이 만들어집니다.
    clock.advance(chrono::Duration::minutes(16));
    let after = s.ensure("emp-1", "process_refund", &a, ttl).await.unwrap();
    assert_eq!(after.status, Status::Pending);
    assert_ne!(after.id, req.id);
    assert_eq!(s.get(&req.id).await.unwrap().status, Status::Expired);
}

/// 만료 경계는 배타적입니다 — 정확히 만료 시각인 순간은 아직 유효합니다.
async fn expiry_boundary_is_exclusive(make: &ApprovalFactory) {
    let clock = new_clock();
    let s = make(clock.clone()).await;
    let a = args(json!({"invoice_id": "INV-1"}));
    let ttl = Duration::from_secs(3600);

    let req = s.ensure("emp-1", "process_refund", &a, ttl).await.unwrap();
    s.decide(&req.id, true, "manager-01", "").await.unwrap();

    // 정확히 TTL 만큼 흐른 순간.
    clock.advance(chrono::Duration::hours(1));
    let got = s.ensure("emp-1", "process_refund", &a, ttl).await.unwrap();
    assert_eq!(
        got.status,
        Status::Approved,
        "만료 경계가 포함적입니다 — 정확히 만료 시각인 순간은 아직 유효해야 합니다"
    );
}

/// TTL 을 주지 않아도 기본값이 붙습니다. **무기한 유효한 승인은 없습니다.**
async fn default_ttl_is_applied(make: &ApprovalFactory) {
    let clock = new_clock();
    let s = make(clock.clone()).await;
    let req = s.ensure("emp-1", "create_support_ticket", &args(json!({})), Duration::ZERO).await.unwrap();
    assert_eq!(req.ttl, approval::DEFAULT_TTL);

    let decided = s.decide(&req.id, true, "manager-01", "").await.unwrap();
    assert!(decided.expires_at.is_some(), "TTL 0 으로 요청해도 만료 시각이 있어야 합니다");
}

/// 거부된 요청은 만료될 것이 없고, 인자가 같으면 계속 거부 상태입니다.
async fn rejected_requests_never_expire(make: &ApprovalFactory) {
    let clock = new_clock();
    let s = make(clock.clone()).await;
    let a = args(json!({"invoice_id": "INV-1"}));

    let req = s.ensure("emp-1", "process_refund", &a, Duration::ZERO).await.unwrap();
    let decided = s.decide(&req.id, false, "manager-01", "근거 부족").await.unwrap();
    assert_eq!(decided.status, Status::Rejected);
    assert!(decided.expires_at.is_none());

    // 같은 인자로 다시 불러도 거부 상태를 유지합니다 (조용히 새 대기 요청을 만들지
    // 않음).
    let again = s.ensure("emp-1", "process_refund", &a, Duration::ZERO).await.unwrap();
    assert_eq!(again.status, Status::Rejected);
    assert_eq!(again.id, req.id);
}

/// 이미 결정된 요청은 다시 결정할 수 없습니다.
async fn a_decided_request_cannot_be_decided_again(make: &ApprovalFactory) {
    let clock = new_clock();
    let s = make(clock.clone()).await;
    let req = s.ensure("emp-1", "create_support_ticket", &args(json!({})), Duration::ZERO).await.unwrap();
    s.decide(&req.id, true, "manager-01", "").await.unwrap();

    let err = s.decide(&req.id, false, "manager-02", "").await.unwrap_err();
    assert!(matches!(err, approval::Error::NotPending));
}

async fn missing_request_is_not_found(make: &ApprovalFactory) {
    let clock = new_clock();
    let s = make(clock.clone()).await;
    let err = s.get("req_nope").await.unwrap_err();
    assert!(matches!(err, approval::Error::NotFound));
}

async fn list_filters_by_status(make: &ApprovalFactory) {
    let clock = new_clock();
    let s = make(clock.clone()).await;

    let a = s.ensure("emp-1", "tool-a", &args(json!({})), Duration::ZERO).await.unwrap();
    s.ensure("emp-1", "tool-b", &args(json!({})), Duration::ZERO).await.unwrap();
    s.decide(&a.id, false, "manager-01", "").await.unwrap();

    let pending = s.list(Some(Status::Pending), 10).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tool, "tool-b");

    let all = s.list(None, 10).await.unwrap();
    assert_eq!(all.len(), 2);
}
