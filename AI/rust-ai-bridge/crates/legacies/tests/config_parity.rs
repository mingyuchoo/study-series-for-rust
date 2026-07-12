//! **재구성 검증.**
//!
//! `go-legacies` 원본 소스가 없으므로 어댑터를 재구성했습니다. 그것이 옳다는
//! 근거는 이 검사입니다 — 실제 `config/*.yaml` 이 요구하는 표면과 정확히
//! 맞아야만 통과합니다.
//!
//! 정책은 도구 이름·인자 이름·출력 필드 이름을 **직접 참조**하고,
//! [`policy::validate_references`] 는 그중 하나라도 실재하지 않으면 기동을
//! 거부합니다. 즉 어댑터를 잘못 재구성했다면 이 검사가 반드시 실패합니다.

use ai_bridge::{SystemsOptions,
                app,
                auth,
                inventory::Inventory,
                policy,
                registry::{Access,
                           RiskLevel}};
use legacies::build_adapters;
use std::{path::{Path,
                 PathBuf},
          sync::Arc};

fn config(name: &str) -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config").join(name) }

async fn registry_and_inventory() -> (ai_bridge::registry::Registry, Arc<Inventory>) {
    let inv = Arc::new(Inventory::load(&config("systems.yaml")).unwrap());
    let opts = SystemsOptions {
        inventory: Some(inv.clone()),
        ..Default::default()
    };
    let adapters = build_adapters(&opts, None).await.unwrap();
    let reg = app::build_registry(&adapters).unwrap();
    (reg, inv)
}

/// 등록된 도구가 `systems.yaml` 과 일치해야 합니다.
#[tokio::test]
async fn registry_matches_the_real_inventory() {
    let (reg, inv) = registry_and_inventory().await;
    app::validate_inventory(&reg, &inv).expect("인벤토리와 도구가 어긋납니다");

    // 인벤토리의 여섯 시스템이 모두 쓰입니다.
    assert!(
        app::unused_systems(&reg, &inv).is_empty(),
        "도구가 없는 시스템이 있습니다: {:?}",
        app::unused_systems(&reg, &inv)
    );
}

/// **가장 중요한 검사.** 실제 정책이 참조하는 모든 것이 실재해야 합니다.
///
/// `policies.yaml` 은 `get_customer_profile` 의 출력 필드
/// `rrn`·`phone`·`email`, `search_contracts` 의 `amount`·`signed_at`·`keyword`,
/// `submit_purchase_request` 의 `amount`, `draft_purchase_request` 의
/// `item`·`quantity` 등을 직접 가리킵니다. 어댑터가 그 이름을 하나라도 다르게
/// 냈다면 여기서 실패합니다.
#[tokio::test]
async fn the_real_policy_file_validates_against_the_reconstructed_adapters() {
    let (reg, inv) = registry_and_inventory().await;
    let engine = policy::Engine::load(&config("policies.yaml")).expect("정책 파일이 유효해야 합니다");

    policy::validate_references(&engine.snapshot(), &reg, &inv).expect("정책이 참조하는 도구·인자·출력 필드가 어댑터에 없습니다");
}

/// `principal.yaml` 의 에이전트 스코프가 실재하는 도구·시스템을 가리켜야
/// 합니다.
#[tokio::test]
async fn the_real_principal_file_validates_against_the_registry() {
    let (reg, inv) = registry_and_inventory().await;
    let dir = auth::load_directory(&config("principal.yaml")).expect("주체 파일이 유효해야 합니다");

    policy::validate_allowlists(&dir.identities(), &reg, &inv).expect("주체의 allowed_tools/allowed_systems 가 실재하지 않습니다");
}

/// 역할이 요구하는 권한이 전부 어떤 도구엔가 존재해야 합니다.
///
/// `validate_references` 가 이것을 검사하지만, 실패했을 때 원인을 바로 보이도록
/// 따로 둡니다.
#[tokio::test]
async fn every_role_permission_is_granted_by_some_tool() {
    let (reg, _) = registry_and_inventory().await;
    let engine = policy::Engine::load(&config("policies.yaml")).unwrap();
    let cfg = engine.snapshot();

    let granted: std::collections::HashSet<String> = reg.specs().into_iter().flat_map(|s| s.required_permissions).collect();

    for (role, r) in &cfg.roles {
        for p in &r.permissions {
            if p == "*" {
                continue;
            }
            assert!(granted.contains(p), "역할 {role:?} 의 권한 {p:?} 을(를) 요구하는 도구가 없습니다");
        }
    }
}

/// README 의 도구 표와 등급·접근이 정확히 일치해야 합니다.
#[tokio::test]
async fn risk_levels_match_the_documented_table() {
    let (reg, _) = registry_and_inventory().await;

    let want: Vec<(&str, &str, Access, RiskLevel)> = vec![
        ("get_invoice_status", "erp", Access::Read, RiskLevel::L1),
        ("get_customer_invoices", "erp", Access::Read, RiskLevel::L1),
        ("get_customer_profile", "crm", Access::Read, RiskLevel::L1),
        ("search_contracts", "crm", Access::Read, RiskLevel::L1),
        ("get_ticket_status", "ticket", Access::Read, RiskLevel::L1),
        ("search_documents", "docs", Access::Read, RiskLevel::L1),
        ("draft_purchase_request", "purchase", Access::Write, RiskLevel::L2),
        ("create_support_ticket", "ticket", Access::Write, RiskLevel::L3),
        ("submit_purchase_request", "purchase", Access::Write, RiskLevel::L3),
        ("get_workflow_status", "refund", Access::Read, RiskLevel::L1),
        ("process_refund", "refund", Access::Write, RiskLevel::L4),
    ];

    for (name, system, access, risk) in want {
        let spec = reg.lookup(name).unwrap_or_else(|| panic!("도구 {name} 이(가) 없습니다")).spec;
        assert_eq!(spec.system, system, "{name}: 시스템이 다릅니다");
        assert_eq!(spec.access, access, "{name}: 접근이 다릅니다");
        assert_eq!(spec.risk_level, risk, "{name}: 위험 등급이 다릅니다");
    }
}

/// 승인 유효 기간이 README 표와 일치해야 합니다.
#[tokio::test]
async fn approval_ttls_match_the_documented_table() {
    use std::time::Duration;
    let (reg, _) = registry_and_inventory().await;

    // 되돌리기 가장 어려운 것이 가장 짧습니다.
    assert_eq!(reg.lookup("process_refund").unwrap().spec.approval_ttl, Duration::from_secs(15 * 60));
    assert_eq!(reg.lookup("create_support_ticket").unwrap().spec.approval_ttl, Duration::from_secs(4 * 3600));
    assert_eq!(reg.lookup("submit_purchase_request").unwrap().spec.approval_ttl, Duration::from_secs(24 * 3600));
}
