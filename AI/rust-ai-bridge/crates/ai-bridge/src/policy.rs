//! 정책 엔진 — RBAC + ABAC + 의무.
//!
//! **RBAC**는 역할 → 권한을 매핑하고, **ABAC 규칙**은 속성과 인자를 근거로
//! 판단합니다. 규칙 하나는 적용 범위와 술어 목록을 가지며, **술어는 모두 참일
//! 때만(AND) 발동합니다.** OR가 필요하면 규칙을 둘로 나눕니다.
//!
//! **결측값 처리는 방향이 다릅니다.** 부정형 술어(`not_equals`, `not_in`,
//! `not_in_attribute`)는 값이 없으면 참으로 평가합니다 — `network_zone` 속성이
//! 없는 주체는 "사내망이 아니다"로 보고 거부합니다(fail-closed). 반대로 숫자
//! 비교와 `equals`/`in` 은 값이 없으면 발동하지 않습니다 — 필수 인자 여부는
//! 입력 스키마가 이미 보장하기 때문입니다.

use crate::{auth::Identity,
            inventory::Inventory,
            registry::{Registry,
                       RiskLevel,
                       Spec}};
use anyhow::{Result,
             anyhow,
             bail};
use serde::{Deserialize,
            Serialize};
use serde_json::Value;
use sha2::{Digest,
           Sha256};
use std::{collections::{HashMap,
                        HashSet},
          path::Path,
          sync::RwLock};

/// 규칙이 발동했을 때 무엇을 할지.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Effect {
    /// 호출 거부 (생략 시 기본값).
    #[default]
    #[serde(rename = "deny")]
    Deny,
    /// 허용하되 사람 승인을 요구 — 낮은 등급을 조건부로 승인 대상으로 올립니다.
    #[serde(rename = "require_approval")]
    RequireApproval,
    /// 허용하되 출력을 좁힙니다 — 허용/거부로 표현할 수 없는 정책.
    #[serde(rename = "allow_with_obligations")]
    AllowWithObligations,
}

/// 출력에 부과되는 의무.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obligations {
    /// 값은 있지만 가려짐 — 연락처·금액처럼 존재는 알려도 되는 것.
    #[serde(default)]
    pub mask_fields: Vec<String>,
    /// 값 자체가 없음 — 계약서 원문처럼 일부만 가려도 의미가 남는 것.
    #[serde(default)]
    pub redact_fields: Vec<String>,
    /// 배열 원소 수 제한. 잘리면 `truncated: true` 가 붙습니다.
    #[serde(default)]
    pub max_rows: i64,
}

impl Obligations {
    pub fn is_zero(&self) -> bool { self.mask_fields.is_empty() && self.redact_fields.is_empty() && self.max_rows == 0 }

    /// 여러 의무 규칙이 걸리면 **가장 좁은 결과**가 나옵니다.
    ///
    /// 필드 목록은 합집합, `max_rows` 는 **0이 아닌 값들 중 최솟값**이
    /// 이깁니다. 0은 "제한 없음"이므로 이미 설정된 제한을 덮어쓰지
    /// 않습니다.
    pub fn merge(&self, other: &Obligations) -> Obligations {
        // 0 은 "제한 없음"이므로 최솟값 계산에서 빼고, 둘 다 0이면 0(무제한)입니다.
        let max_rows = [self.max_rows, other.max_rows].into_iter().filter(|n| *n > 0).min().unwrap_or(0);
        Obligations {
            mask_fields: union(&self.mask_fields, &other.mask_fields),
            redact_fields: union(&self.redact_fields, &other.redact_fields),
            max_rows,
        }
    }
}

/// 순서를 보존하는 합집합.
fn union(a: &[String], b: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for s in a.iter().chain(b.iter()) {
        if seen.insert(s.clone()) {
            out.push(s.clone());
        }
    }
    out
}

/// 정책 판정 결과.
#[derive(Debug, Clone, Default)]
pub struct Decision {
    pub allowed: bool,
    pub reason: String,
    pub rule_id: String,
    pub approval_required: bool,
    pub obligations: Obligations,
    /// 발동한 모든 규칙 ID (감사용).
    pub matched_rules: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// 술어 — 정확히 하나의 대상(`attribute` XOR `arg`)과 정확히 하나의 연산자를
/// 가집니다.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Predicate {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub attribute: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub arg: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_equals: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub r#in: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_in: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gt: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gte: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lt: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lte: Option<f64>,
    /// 인자 값이 주체 속성 목록 **안에** 있어야 함.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub in_attribute: String,
    /// 인자 값이 주체 속성 목록 **밖에** 있어야 함.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub not_in_attribute: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub effect: Effect,
    #[serde(default)]
    pub applies_to_tools: Vec<String>,
    #[serde(default)]
    pub applies_to_roles: Vec<String>,
    #[serde(default)]
    pub applies_to_systems: Vec<String>,
    /// AND 로 결합됩니다.
    #[serde(default)]
    pub when: Vec<Predicate>,
    #[serde(default)]
    pub obligations: Obligations,
}

impl Rule {
    fn reason(&self) -> String {
        if self.description.is_empty() {
            format!("정책 규칙 {:?} 에 의해 거부되었습니다", self.id)
        } else {
            self.description.clone()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)] // Go 의 KnownFields(true) — 오타 난 필드를 조용히 무시하지 않습니다.
pub struct Config {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub roles: HashMap<String, Role>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

/// 정책 엔진 (핫 리로드 + 1단계 롤백).
#[derive(Debug, Default)]
pub struct Engine {
    inner: RwLock<EngineState>,
}

#[derive(Debug, Default)]
struct EngineState {
    cfg: Config,
    digest: String,
    prev: Option<(Config, String)>,
}

impl Engine {
    pub fn new(cfg: Config) -> Self {
        Self {
            inner: RwLock::new(EngineState {
                cfg,
                digest: String::new(),
                prev: None,
            }),
        }
    }

    /// 정책 파일을 읽고 구조 검증까지 합니다.
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read(path).map_err(|e| anyhow!("read policy file: {e}"))?;
        let cfg = parse(&data)?;
        validate(&cfg).map_err(|e| anyhow!("invalid policy file {}: {e}", path.display()))?;
        if cfg.version.is_empty() {
            bail!("invalid policy file {}: version is required", path.display());
        }
        Ok(Self {
            inner: RwLock::new(EngineState {
                cfg,
                digest: digest_of(&data),
                prev: None,
            }),
        })
    }

    pub fn version(&self) -> (String, String) {
        let st = self.inner.read().unwrap();
        (st.cfg.version.clone(), st.digest.clone())
    }

    pub fn snapshot(&self) -> Config { self.inner.read().unwrap().cfg.clone() }

    /// 파일을 다시 읽어 적용합니다.
    ///
    /// 구조 검증에 더해 **레지스트리·인벤토리와의 교차 검증**까지 통과해야
    /// 합니다. 어느 단계든 실패하면 활성 정책은 그대로입니다.
    pub fn reload(&self, path: &Path, reg: &Registry, inv: &Inventory) -> Result<(String, String)> {
        let data = std::fs::read(path).map_err(|e| anyhow!("read policy file: {e}"))?;
        let cfg = parse(&data)?;
        validate(&cfg)?;
        if cfg.version.is_empty() {
            bail!("version is required");
        }
        validate_references(&cfg, reg, inv)?;

        let new_digest = digest_of(&data);
        let mut st = self.inner.write().unwrap();
        let old_version = st.cfg.version.clone();
        st.prev = Some((st.cfg.clone(), st.digest.clone()));
        st.cfg = cfg;
        st.digest = new_digest;
        Ok((old_version, st.cfg.version.clone()))
    }

    /// 직전 정책으로 되돌립니다 (swap 이므로 두 번 하면 원래대로).
    pub fn rollback(&self) -> Result<(String, String)> {
        let mut st = self.inner.write().unwrap();
        let Some((prev_cfg, prev_digest)) = st.prev.take() else {
            bail!("no previous policy version");
        };
        let old_version = st.cfg.version.clone();
        let current = (st.cfg.clone(), st.digest.clone());
        st.cfg = prev_cfg;
        st.digest = prev_digest;
        st.prev = Some(current);
        Ok((old_version, st.cfg.version.clone()))
    }

    /// 도구가 목록에 보여도 되는지 (RBAC + 주체 스코프만).
    ///
    /// **ABAC 규칙은 보지 않습니다** — 시각·네트워크·인자에 의존하므로 목록
    /// 시점에는 알 수 없습니다. 이것은 **광고 축소일 뿐 보안 경계가
    /// 아닙니다.** 실제 집행은 언제나 호출 시점의 [`Engine::evaluate`] 가
    /// 합니다.
    pub fn visible(&self, id: &Identity, spec: &Spec) -> bool {
        let st = self.inner.read().unwrap();
        let perms = effective_permissions(&st.cfg, id);
        if !spec.required_permissions.iter().all(|need| has_permission(&perms, need)) {
            return false;
        }
        id.allows_tool(&spec.name, &spec.system)
    }

    /// 호출을 판정합니다.
    ///
    /// 순서: **RBAC → 주체 스코프 → ABAC**. ABAC 에서 **첫 deny 가 즉시
    /// 이깁니다.**
    pub fn evaluate(&self, id: &Identity, spec: &Spec, args: &serde_json::Map<String, Value>) -> Decision {
        let st = self.inner.read().unwrap();
        let cfg = &st.cfg;

        // 1. RBAC — 필요한 권한이 없으면 즉시 거부.
        let perms = effective_permissions(cfg, id);
        for need in &spec.required_permissions {
            if !has_permission(&perms, need) {
                return Decision {
                    allowed: false,
                    reason: format!("권한 부족: 필요 권한 {need:?} 이(가) 없습니다"),
                    rule_id: "rbac.missing_permission".into(),
                    ..Default::default()
                };
            }
        }

        // 2. 주체 스코프 (에이전트 allowlist) — 역할이 허용해도 목록 밖이면 거부.
        if !id.allows_tool(&spec.name, &spec.system) {
            return Decision {
                allowed: false,
                reason: format!("주체 {:?} 의 허용 범위 밖입니다(도구 {:?}, 시스템 {:?})", id.user_id, spec.name, spec.system),
                rule_id: "principal.out_of_scope".into(),
                ..Default::default()
            };
        }

        // 3. ABAC — 선언 순서대로.
        let mut matched: Vec<String> = Vec::new();
        let mut obligations = Obligations::default();
        let mut escalated_by = String::new();

        for rule in &cfg.rules {
            if !rule_applies(rule, id, spec) || !rule_fires(rule, id, args) {
                continue;
            }
            matched.push(rule.id.clone());
            match rule.effect {
                | Effect::Deny => {
                    // 첫 deny 가 즉시 이깁니다 — 뒤에 의무·승인 규칙이 있어도 보지 않습니다.
                    return Decision {
                        allowed: false,
                        reason: rule.reason(),
                        rule_id: rule.id.clone(),
                        matched_rules: matched,
                        ..Default::default()
                    };
                },
                | Effect::AllowWithObligations => {
                    // 누적 병합 — 순회를 멈추지 않습니다.
                    obligations = obligations.merge(&rule.obligations);
                },
                | Effect::RequireApproval => {
                    // 사유에는 **첫** 규칙만 남기되 순회는 계속합니다.
                    if escalated_by.is_empty() {
                        escalated_by = rule.id.clone();
                    }
                },
            }
        }

        // 4. 승인 필요 여부. L2 는 등급만으로는 승인 대상이 아닙니다.
        let by_risk = spec.risk_level >= RiskLevel::L3;
        let approval = spec.approval_required || by_risk || !escalated_by.is_empty();

        let reason = if !escalated_by.is_empty() {
            format!("허용(규칙 {escalated_by:?} 에 의해 승인 필요)")
        } else if by_risk || spec.approval_required {
            "허용(위험 등급에 따라 승인 필요)".to_string()
        } else {
            "허용".to_string()
        };

        Decision {
            allowed: true,
            reason,
            rule_id: "default.allow".into(),
            approval_required: approval,
            obligations,
            matched_rules: matched,
        }
    }
}

fn parse(data: &[u8]) -> Result<Config> {
    let s = std::str::from_utf8(data)?;
    serde_norway::from_str(s).map_err(|e| anyhow!("parse policy file: {e}"))
}

fn digest_of(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

/// 직접 부여된 권한 ∪ 역할이 주는 권한. 모르는 역할은 조용히 무시합니다.
fn effective_permissions(cfg: &Config, id: &Identity) -> HashSet<String> {
    let mut out: HashSet<String> = id.permissions.iter().cloned().collect();
    for role in &id.roles {
        if let Some(r) = cfg.roles.get(role) {
            out.extend(r.permissions.iter().cloned());
        }
    }
    out
}

/// `"*"` 는 모든 권한을 줍니다.
fn has_permission(perms: &HashSet<String>, need: &str) -> bool { perms.contains("*") || perms.contains(need) }

/// 규칙의 적용 범위가 이 호출을 덮는지. **빈 목록 = 그 축에는 제한 없음.**
fn rule_applies(rule: &Rule, id: &Identity, spec: &Spec) -> bool {
    if !rule.applies_to_tools.is_empty() && !rule.applies_to_tools.contains(&spec.name) {
        return false;
    }
    if !rule.applies_to_systems.is_empty() && !rule.applies_to_systems.contains(&spec.system) {
        return false;
    }
    if !rule.applies_to_roles.is_empty() && !rule.applies_to_roles.iter().any(|r| id.has_role(r)) {
        return false;
    }
    true
}

/// 모든 술어가 참일 때만(AND) 규칙이 발동합니다. 술어가 없으면 무조건
/// 발동합니다.
fn rule_fires(rule: &Rule, id: &Identity, args: &serde_json::Map<String, Value>) -> bool { rule.when.iter().all(|p| predicate_matches(p, id, args)) }

fn predicate_matches(p: &Predicate, id: &Identity, args: &serde_json::Map<String, Value>) -> bool {
    // --- in_attribute / not_in_attribute: 인자 값을 주체 속성 목록과 대조 ---
    if !p.in_attribute.is_empty() || !p.not_in_attribute.is_empty() {
        let attr_name = if p.in_attribute.is_empty() { &p.not_in_attribute } else { &p.in_attribute };
        let list = id.attr(attr_name).map(to_string_slice).unwrap_or_default();
        let arg_val = args.get(&p.arg);
        let in_list = arg_val.map(|v| list.contains(&to_string(v))).unwrap_or(false);

        if !p.in_attribute.is_empty() {
            return in_list;
        }
        // 결측 인자는 "목록에 없다"로 봅니다(fail-closed).
        return !in_list;
    }

    // --- 그 외: attribute 또는 arg 에서 값을 꺼냅니다 ---
    let value: Option<&Value> = if !p.attribute.is_empty() { id.attr(&p.attribute) } else { args.get(&p.arg) };

    if let Some(want) = &p.equals {
        // 결측 → 거짓.
        return value.map(|v| to_string(v) == *want).unwrap_or(false);
    }
    if let Some(want) = &p.not_equals {
        // 결측 → 참 (fail-closed: 속성이 없는 주체는 "internal 이 아니다").
        return value.map(|v| to_string(v) != *want).unwrap_or(true);
    }
    if !p.r#in.is_empty() {
        return value.map(|v| p.r#in.contains(&to_string(v))).unwrap_or(false);
    }
    if !p.not_in.is_empty() {
        // 결측 → 참 (fail-closed).
        return value.map(|v| !p.not_in.contains(&to_string(v))).unwrap_or(true);
    }

    // --- 숫자 비교: 결측이거나 숫자가 아니면 **발동하지 않습니다** ---
    // 필수 인자 여부는 입력 스키마가 이미 보장합니다.
    let Some(n) = value.and_then(to_float) else {
        return false;
    };
    if let Some(x) = p.gt {
        return n > x;
    }
    if let Some(x) = p.gte {
        return n >= x;
    }
    if let Some(x) = p.lt {
        return n < x;
    }
    if let Some(x) = p.lte {
        return n <= x;
    }
    false
}

/// Go 의 `fmt.Sprintf("%v", v)` 와 같은 문자열화.
///
/// 정책 YAML 의 `equals: "false"` 가 Enricher 가 넣은 불리언·문자열과
/// 대조되므로, 불리언은 `true`/`false`, 정수는 소수점 없이 나와야 합니다.
fn to_string(v: &Value) -> String {
    match v {
        | Value::String(s) => s.clone(),
        | Value::Bool(b) => b.to_string(),
        | Value::Number(n) =>
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else {
                n.to_string()
            },
        | Value::Null => "null".to_string(),
        | other => other.to_string(),
    }
}

fn to_float(v: &Value) -> Option<f64> {
    match v {
        | Value::Number(n) => n.as_f64(),
        | _ => None,
    }
}

fn to_string_slice(v: &Value) -> Vec<String> {
    match v {
        | Value::Array(items) => items.iter().map(to_string).collect(),
        | _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// 검증
// ---------------------------------------------------------------------------

/// 구조 검증 (레지스트리 없이).
pub fn validate(cfg: &Config) -> Result<()> {
    for (name, role) in &cfg.roles {
        if name.trim().is_empty() || role.permissions.is_empty() {
            bail!("role {name:?}: name and at least one permission are required");
        }
    }
    let mut seen = HashSet::new();
    for (i, rule) in cfg.rules.iter().enumerate() {
        if rule.id.is_empty() {
            bail!("rule[{i}]: id is required");
        }
        if !seen.insert(&rule.id) {
            bail!("rule {:?}: duplicate rule id", rule.id);
        }
        match rule.effect {
            | Effect::Deny | Effect::RequireApproval => {
                if !rule.obligations.is_zero() {
                    bail!("rule {:?}: obligations require effect allow_with_obligations", rule.id);
                }
                // 술어 없는 deny 는 범위 전체를 거부하게 됩니다.
                if rule.when.is_empty() {
                    bail!("rule {:?}: when must have at least one predicate", rule.id);
                }
            },
            | Effect::AllowWithObligations => {
                if rule.obligations.is_zero() {
                    bail!("rule {:?}: allow_with_obligations requires at least one obligation", rule.id);
                }
                if rule.obligations.max_rows < 0 {
                    bail!("rule {:?}: max_rows cannot be negative", rule.id);
                }
                // when 은 생략 가능 — 범위 전체에 무조건 의무를 겁니다.
            },
        }
        for (j, p) in rule.when.iter().enumerate() {
            validate_predicate(p).map_err(|e| anyhow!("rule {:?}: when[{j}]: {e}", rule.id))?;
        }
    }
    Ok(())
}

fn validate_predicate(p: &Predicate) -> Result<()> {
    let sources = [!p.attribute.is_empty(), !p.arg.is_empty()].iter().filter(|b| **b).count();
    if sources != 1 {
        bail!("exactly one of attribute/arg must be set");
    }
    let ops = [
        p.equals.is_some(),
        p.not_equals.is_some(),
        !p.r#in.is_empty(),
        !p.not_in.is_empty(),
        p.gt.is_some(),
        p.gte.is_some(),
        p.lt.is_some(),
        p.lte.is_some(),
        !p.in_attribute.is_empty(),
        !p.not_in_attribute.is_empty(),
    ]
    .iter()
    .filter(|b| **b)
    .count();
    if ops != 1 {
        bail!("exactly one operator must be set, got {ops}");
    }
    if (!p.in_attribute.is_empty() || !p.not_in_attribute.is_empty()) && p.arg.is_empty() {
        bail!("in_attribute/not_in_attribute require arg");
    }
    Ok(())
}

/// 레지스트리·인벤토리와 교차 검증합니다.
///
/// 오타 난 도구 이름은 규칙을 **조용히 발동하지 않게** 만듭니다 — 정책이 있는
/// 줄 알았는데 아무것도 막지 않는 상태가 가장 위험합니다. 그래서 기동 시
/// 거부합니다.
pub fn validate_references(cfg: &Config, reg: &Registry, inv: &Inventory) -> Result<()> {
    let specs = reg.specs();
    let tools: HashMap<&str, &Spec> = specs.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut permissions: HashSet<&str> = HashSet::new();
    permissions.insert("*");
    for s in &specs {
        for p in &s.required_permissions {
            permissions.insert(p.as_str());
        }
    }
    for (name, role) in &cfg.roles {
        for p in &role.permissions {
            if !permissions.contains(p.as_str()) {
                bail!("role {name:?}: unknown permission {p:?}");
            }
        }
    }

    for rule in &cfg.rules {
        for r in &rule.applies_to_roles {
            if !cfg.roles.contains_key(r) {
                bail!("rule {:?}: unknown role {r:?}", rule.id);
            }
        }
        for s in &rule.applies_to_systems {
            if inv.lookup(s).is_none() {
                bail!("rule {:?}: unknown system {s:?}", rule.id);
            }
        }
        for t in &rule.applies_to_tools {
            if !tools.contains_key(t.as_str()) {
                bail!("rule {:?}: unknown tool {t:?}", rule.id);
            }
        }

        if rule.applies_to_tools.is_empty() {
            // 도구를 특정하지 않으면 인자·출력 필드를 스키마로 확인할 수 없습니다.
            if rule.when.iter().any(|p| !p.arg.is_empty()) {
                bail!("rule {:?}: arg predicate requires applies_to_tools for schema validation", rule.id);
            }
            if !rule.obligations.is_zero() {
                bail!("rule {:?}: obligations require applies_to_tools for output validation", rule.id);
            }
            continue;
        }

        for t in &rule.applies_to_tools {
            let spec = tools[t.as_str()];
            let input_fields = schema_fields(&spec.input_schema);
            let output_fields = schema_fields(&spec.output_schema);

            for p in &rule.when {
                if !p.arg.is_empty() && !input_fields.contains(&p.arg) {
                    bail!("rule {:?}: tool {t:?} has no input field {:?}", rule.id, p.arg);
                }
            }
            for f in rule.obligations.mask_fields.iter().chain(rule.obligations.redact_fields.iter()) {
                if !output_fields.contains(f) {
                    bail!("rule {:?}: tool {t:?} has no output field {f:?}", rule.id);
                }
            }
        }
    }
    Ok(())
}

/// 스키마 트리 전체에서 property 이름을 모읍니다 (중첩 포함, 경로는 구분하지
/// 않음).
fn schema_fields(s: &Value) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_fields(s, &mut out);
    out
}

fn collect_fields(s: &Value, out: &mut HashSet<String>) {
    let Some(obj) = s.as_object() else {
        return;
    };
    if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
        for (name, child) in props {
            out.insert(name.clone());
            collect_fields(child, out);
            if let Some(items) = child.get("items") {
                collect_fields(items, out);
            }
        }
    }
    if let Some(items) = obj.get("items") {
        collect_fields(items, out);
    }
}

/// 주체의 `allowed_tools`/`allowed_systems` 가 실재하는 것을 가리키는지
/// 검증합니다.
pub fn validate_allowlists(ids: &[Identity], reg: &Registry, inv: &Inventory) -> Result<()> {
    let names: HashSet<String> = reg.names().into_iter().collect();
    for id in ids {
        for t in &id.allowed_tools {
            if !names.contains(t) {
                bail!("주체 {:?}: 허용 도구 {t:?} 이(가) 레지스트리에 없습니다", id.user_id);
            }
        }
        for s in &id.allowed_systems {
            if inv.lookup(s).is_none() {
                bail!("주체 {:?}: 허용 시스템 {s:?} 이(가) 인벤토리에 없습니다", id.user_id);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Access,
                          Sensitivity};
    use serde_json::json;

    fn args(v: Value) -> serde_json::Map<String, Value> { v.as_object().unwrap().clone() }

    fn spec(name: &str, risk: RiskLevel) -> Spec {
        Spec {
            name: name.into(),
            description: "d".into(),
            system: "crm".into(),
            access: Access::Read,
            risk_level: risk,
            sensitivity: Sensitivity::Internal,
            required_permissions: vec!["crm.customer.read".into()],
            rate_limit_per_min: 60,
            log_retention_days: 90,
            fallback: "f".into(),
            input_schema: json!({"type":"object"}),
            output_schema: json!({"type":"object"}),
            ..Default::default()
        }
    }

    fn sales() -> Identity {
        Identity {
            user_id: "emp-sales-01".into(),
            roles: vec!["sales".into()],
            attributes: HashMap::from([("managed_customers".into(), json!(["CUST-1001", "CUST-1002"]))]),
            ..Default::default()
        }
    }

    fn cfg_with(rules: Vec<Rule>) -> Config {
        Config {
            version: "t".into(),
            roles: HashMap::from([(
                "sales".to_string(),
                Role {
                    permissions: vec!["crm.customer.read".into()],
                },
            )]),
            rules,
        }
    }

    // --- RBAC ---

    #[test]
    fn denies_when_permission_is_missing() {
        let e = Engine::new(cfg_with(vec![]));
        let mut id = sales();
        id.roles = vec!["hr".into()];
        let d = e.evaluate(&id, &spec("get_customer_profile", RiskLevel::L1), &args(json!({})));
        assert!(!d.allowed);
        assert_eq!(d.rule_id, "rbac.missing_permission");
    }

    #[test]
    fn wildcard_permission_grants_everything() {
        let mut cfg = cfg_with(vec![]);
        cfg.roles.insert(
            "admin".into(),
            Role {
                permissions: vec!["*".into()],
            },
        );
        let e = Engine::new(cfg);
        let id = Identity {
            user_id: "admin-1".into(),
            roles: vec!["admin".into()],
            ..Default::default()
        };
        assert!(e.evaluate(&id, &spec("get_customer_profile", RiskLevel::L1), &args(json!({}))).allowed);
    }

    #[test]
    fn agent_scope_narrows_even_when_rbac_allows() {
        let e = Engine::new(cfg_with(vec![]));
        let mut id = sales();
        id.allowed_tools = vec!["some_other_tool".into()];
        let d = e.evaluate(&id, &spec("get_customer_profile", RiskLevel::L1), &args(json!({})));
        assert!(!d.allowed);
        assert_eq!(d.rule_id, "principal.out_of_scope");
    }

    // --- 승인 상향 ---

    #[test]
    fn l3_requires_approval_by_risk_alone() {
        let e = Engine::new(cfg_with(vec![]));
        let mut s = spec("create_support_ticket", RiskLevel::L3);
        s.access = Access::Write;
        let d = e.evaluate(&sales(), &s, &args(json!({})));
        assert!(d.allowed);
        assert!(d.approval_required);
    }

    #[test]
    fn l2_does_not_require_approval_by_risk() {
        let e = Engine::new(cfg_with(vec![]));
        let mut s = spec("draft_purchase_request", RiskLevel::L2);
        s.access = Access::Write;
        let d = e.evaluate(&sales(), &s, &args(json!({})));
        assert!(d.allowed);
        assert!(!d.approval_required);
    }

    #[test]
    fn require_approval_rule_escalates_an_l2_tool() {
        let rule = Rule {
            id: "bulk-laptop".into(),
            effect: Effect::RequireApproval,
            applies_to_tools: vec!["draft_purchase_request".into()],
            when: vec![
                Predicate {
                    arg: "item".into(),
                    equals: Some("노트북".into()),
                    ..Default::default()
                },
                Predicate {
                    arg: "quantity".into(),
                    gte: Some(10.0),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let e = Engine::new(cfg_with(vec![rule]));
        let s = spec("draft_purchase_request", RiskLevel::L2);

        // 두 술어가 AND — 노트북 10대 이상일 때만 발동.
        let d = e.evaluate(&sales(), &s, &args(json!({"item":"노트북","quantity":10})));
        assert!(d.allowed && d.approval_required);
        assert_eq!(d.matched_rules, vec!["bulk-laptop"]);

        let d = e.evaluate(&sales(), &s, &args(json!({"item":"노트북","quantity":9})));
        assert!(!d.approval_required);
        let d = e.evaluate(&sales(), &s, &args(json!({"item":"모니터","quantity":50})));
        assert!(!d.approval_required);
    }

    // --- deny 우선 ---

    #[test]
    fn first_deny_wins_and_short_circuits() {
        let rules = vec![
            Rule {
                id: "deny-it".into(),
                effect: Effect::Deny,
                applies_to_tools: vec!["get_customer_profile".into()],
                when: vec![Predicate {
                    attribute: "network_zone".into(),
                    not_equals: Some("internal".into()),
                    ..Default::default()
                }],
                ..Default::default()
            },
            Rule {
                id: "obligate-it".into(),
                effect: Effect::AllowWithObligations,
                applies_to_tools: vec!["get_customer_profile".into()],
                obligations: Obligations {
                    mask_fields: vec!["phone".into()],
                    ..Default::default()
                },
                ..Default::default()
            },
        ];
        let e = Engine::new(cfg_with(rules));
        // network_zone 속성이 없음 → not_equals 는 참 → deny 발동.
        let d = e.evaluate(&sales(), &spec("get_customer_profile", RiskLevel::L1), &args(json!({})));
        assert!(!d.allowed);
        assert_eq!(d.rule_id, "deny-it");
        // deny 에서 멈췄으므로 뒤의 의무 규칙은 보지 않았습니다.
        assert_eq!(d.matched_rules, vec!["deny-it"]);
        assert!(d.obligations.is_zero());
    }

    // --- 결측값 방향성 ---

    #[test]
    fn not_equals_fires_on_missing_attribute_fail_closed() {
        // network_zone 이 없는 주체는 "사내망이 아니다"로 봅니다.
        let p = Predicate {
            attribute: "network_zone".into(),
            not_equals: Some("internal".into()),
            ..Default::default()
        };
        assert!(predicate_matches(&p, &sales(), &args(json!({}))));
    }

    #[test]
    fn equals_does_not_fire_on_missing_attribute() {
        let p = Predicate {
            attribute: "network_zone".into(),
            equals: Some("internal".into()),
            ..Default::default()
        };
        assert!(!predicate_matches(&p, &sales(), &args(json!({}))));
    }

    #[test]
    fn not_in_fires_on_missing_but_in_does_not() {
        let base = Predicate {
            attribute: "business_purpose".into(),
            ..Default::default()
        };
        let not_in = Predicate {
            not_in: vec!["customer_support".into()],
            ..base.clone()
        };
        let is_in = Predicate {
            r#in: vec!["customer_support".into()],
            ..base
        };
        assert!(predicate_matches(&not_in, &sales(), &args(json!({}))));
        assert!(!predicate_matches(&is_in, &sales(), &args(json!({}))));
    }

    #[test]
    fn numeric_comparison_never_fires_on_missing_arg() {
        // 필수 인자 여부는 스키마가 보장하므로 정책은 발동하지 않습니다.
        let p = Predicate {
            arg: "amount".into(),
            gte: Some(5_000_000.0),
            ..Default::default()
        };
        assert!(!predicate_matches(&p, &sales(), &args(json!({}))));
        assert!(predicate_matches(&p, &sales(), &args(json!({"amount": 5_000_000}))));
        assert!(!predicate_matches(&p, &sales(), &args(json!({"amount": 4_999_999}))));
    }

    #[test]
    fn not_in_attribute_treats_missing_arg_as_not_in_list() {
        let p = Predicate {
            arg: "customer_id".into(),
            not_in_attribute: "managed_customers".into(),
            ..Default::default()
        };
        // 담당 고객이면 발동하지 않습니다.
        assert!(!predicate_matches(&p, &sales(), &args(json!({"customer_id":"CUST-1001"}))));
        // 담당이 아니면 발동합니다(= deny).
        assert!(predicate_matches(&p, &sales(), &args(json!({"customer_id":"CUST-9999"}))));
        // 인자가 없으면 "목록에 없다"로 봅니다.
        assert!(predicate_matches(&p, &sales(), &args(json!({}))));
    }

    #[test]
    fn booleans_stringify_like_go() {
        // Enricher 는 business_hours 를 문자열로 넣지만, YAML 이 불리언을 줄 수도
        // 있습니다.
        assert_eq!(to_string(&json!(false)), "false");
        assert_eq!(to_string(&json!(true)), "true");
        assert_eq!(to_string(&json!(10)), "10");
        assert_eq!(to_string(&json!("x")), "x");
    }

    // --- 의무 병합 ---

    #[test]
    fn obligations_merge_union_fields_and_min_nonzero_max_rows() {
        let a = Obligations {
            mask_fields: vec!["amount".into()],
            redact_fields: vec!["signed_at".into()],
            max_rows: 3,
        };
        let b = Obligations {
            mask_fields: vec!["amount".into(), "phone".into()],
            max_rows: 1,
            ..Default::default()
        };
        let m = a.merge(&b);
        assert_eq!(m.mask_fields, vec!["amount", "phone"]); // 합집합, 중복 제거
        assert_eq!(m.redact_fields, vec!["signed_at"]);
        assert_eq!(m.max_rows, 1); // 더 좁은 쪽이 이깁니다
    }

    #[test]
    fn zero_max_rows_never_overrides_a_set_limit() {
        let a = Obligations {
            max_rows: 3,
            ..Default::default()
        };
        let b = Obligations::default();
        assert_eq!(a.merge(&b).max_rows, 3);
        assert_eq!(b.merge(&a).max_rows, 3);
    }

    #[test]
    fn multiple_obligation_rules_accumulate_across_the_whole_scan() {
        let rules = vec![
            Rule {
                id: "summary-only".into(),
                effect: Effect::AllowWithObligations,
                applies_to_tools: vec!["search_contracts".into()],
                obligations: Obligations {
                    mask_fields: vec!["amount".into()],
                    redact_fields: vec!["signed_at".into()],
                    max_rows: 3,
                },
                ..Default::default()
            },
            Rule {
                id: "unfiltered-narrowed".into(),
                effect: Effect::AllowWithObligations,
                applies_to_tools: vec!["search_contracts".into()],
                when: vec![Predicate {
                    arg: "keyword".into(),
                    equals: Some(String::new()),
                    ..Default::default()
                }],
                obligations: Obligations {
                    max_rows: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
        ];
        let e = Engine::new(cfg_with(rules));
        let s = spec("search_contracts", RiskLevel::L1);

        let d = e.evaluate(&sales(), &s, &args(json!({"keyword": "x"})));
        assert_eq!(d.obligations.max_rows, 3);

        // 키워드가 비면 두 규칙이 모두 걸려 더 좁은 쪽이 이깁니다.
        let d = e.evaluate(&sales(), &s, &args(json!({"keyword": ""})));
        assert_eq!(d.obligations.max_rows, 1);
        assert_eq!(d.obligations.mask_fields, vec!["amount"]);
        assert_eq!(d.matched_rules.len(), 2);
    }

    #[test]
    fn role_scoped_obligations_differ_by_role() {
        // 같은 도구, 같은 권한이어도 역할에 따라 결과가 달라집니다.
        let rule = Rule {
            id: "support-masked".into(),
            effect: Effect::AllowWithObligations,
            applies_to_roles: vec!["support".into()],
            applies_to_tools: vec!["get_customer_profile".into()],
            obligations: Obligations {
                mask_fields: vec!["phone".into(), "email".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let mut cfg = cfg_with(vec![rule]);
        cfg.roles.insert(
            "support".into(),
            Role {
                permissions: vec!["crm.customer.read".into()],
            },
        );
        let e = Engine::new(cfg);
        let s = spec("get_customer_profile", RiskLevel::L1);

        let support = Identity {
            user_id: "sup-1".into(),
            roles: vec!["support".into()],
            ..Default::default()
        };
        assert_eq!(e.evaluate(&support, &s, &args(json!({}))).obligations.mask_fields, vec!["phone", "email"]);
        // 영업팀은 마스킹되지 않습니다.
        assert!(e.evaluate(&sales(), &s, &args(json!({}))).obligations.is_zero());
    }

    // --- visible ---

    #[test]
    fn visible_checks_rbac_and_scope_but_not_abac() {
        let deny_all = Rule {
            id: "deny".into(),
            effect: Effect::Deny,
            when: vec![Predicate {
                attribute: "network_zone".into(),
                not_equals: Some("internal".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let e = Engine::new(cfg_with(vec![deny_all]));
        let s = spec("get_customer_profile", RiskLevel::L1);
        // ABAC 로는 거부되지만 목록에는 보입니다 — 보안 경계가 아니라 광고 축소입니다.
        assert!(e.visible(&sales(), &s));
        assert!(!e.evaluate(&sales(), &s, &args(json!({}))).allowed);
    }

    // --- 검증 ---

    #[test]
    fn loads_the_real_policies_yaml() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/policies.yaml");
        let e = Engine::load(&path).expect("config/policies.yaml should be valid");
        let (version, digest) = e.version();
        assert_eq!(version, "2026-07-11.1");
        assert_eq!(digest.len(), 64);
        assert_eq!(e.snapshot().rules.len(), 11);
    }

    #[test]
    fn rejects_deny_rule_without_predicates() {
        // 술어 없는 deny 는 범위 전체를 거부합니다.
        let cfg = cfg_with(vec![Rule {
            id: "oops".into(),
            effect: Effect::Deny,
            ..Default::default()
        }]);
        assert!(validate(&cfg).is_err());
    }

    #[test]
    fn allows_obligation_rule_without_predicates() {
        let cfg = cfg_with(vec![Rule {
            id: "always".into(),
            effect: Effect::AllowWithObligations,
            obligations: Obligations {
                mask_fields: vec!["phone".into()],
                ..Default::default()
            },
            ..Default::default()
        }]);
        assert!(validate(&cfg).is_ok());
    }

    #[test]
    fn rejects_obligations_on_deny_rule() {
        let cfg = cfg_with(vec![Rule {
            id: "x".into(),
            effect: Effect::Deny,
            when: vec![Predicate {
                arg: "a".into(),
                equals: Some("b".into()),
                ..Default::default()
            }],
            obligations: Obligations {
                max_rows: 1,
                ..Default::default()
            },
            ..Default::default()
        }]);
        assert!(validate(&cfg).is_err());
    }

    #[test]
    fn rejects_duplicate_rule_ids() {
        let r = || Rule {
            id: "dup".into(),
            effect: Effect::Deny,
            when: vec![Predicate {
                arg: "a".into(),
                equals: Some("b".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(validate(&cfg_with(vec![r(), r()])).is_err());
    }

    #[test]
    fn rejects_predicate_with_two_operators() {
        let p = Predicate {
            arg: "a".into(),
            equals: Some("x".into()),
            gte: Some(1.0),
            ..Default::default()
        };
        assert!(validate_predicate(&p).is_err());
    }

    #[test]
    fn rejects_predicate_with_no_operator_or_no_target() {
        assert!(
            validate_predicate(&Predicate {
                arg: "a".into(),
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate_predicate(&Predicate {
                equals: Some("x".into()),
                ..Default::default()
            })
            .is_err()
        );
    }

    #[test]
    fn rejects_in_attribute_on_an_attribute_target() {
        let p = Predicate {
            attribute: "x".into(),
            in_attribute: "managed_customers".into(),
            ..Default::default()
        };
        assert!(validate_predicate(&p).is_err());
    }

    #[test]
    fn unknown_yaml_field_is_rejected_not_silently_ignored() {
        let yaml = "version: \"1\"\nrules:\n  - id: x\n    efect: deny\n";
        assert!(parse(yaml.as_bytes()).is_err());
    }

    #[test]
    fn schema_fields_walks_nested_objects_and_arrays() {
        let s = json!({
            "type": "object",
            "properties": {
                "customer": {"type":"object","properties":{"rrn":{"type":"string"}}},
                "contracts": {"type":"array","items":{"type":"object","properties":{"amount":{"type":"integer"}}}}
            }
        });
        let f = schema_fields(&s);
        assert!(f.contains("customer") && f.contains("rrn") && f.contains("amount"));
    }
}
