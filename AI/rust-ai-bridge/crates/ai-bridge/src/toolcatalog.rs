//! 동적 도구 카탈로그 (`tools-dynamic.yaml`).
//!
//! 게이트웨이를 다시 빌드하지 않고 도구를 추가합니다. **임의 코드는 로드하지
//! 않습니다** — 실행은 `POST {base_url}/tools/{name}` remote 브리지로만
//! 전달됩니다.
//!
//! 등록되는 도구도 레지스트리의 검증을 똑같이 받습니다. 등급 없는 도구, 스키마
//! 없는 도구, 재시도하는 쓰기 도구는 YAML 로 선언해도 거부됩니다.

use crate::{adapter,
            inventory::Inventory,
            registry::{Access,
                       Registry,
                       RiskLevel,
                       Sensitivity,
                       Spec,
                       Tool}};
use anyhow::{Result,
             anyhow,
             bail};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashSet,
          path::{Path,
                 PathBuf},
          sync::{Arc,
                 Mutex},
          time::Duration};

#[derive(Debug, Deserialize, Default)]
struct FileDoc {
    #[serde(default)]
    #[allow(dead_code)]
    version: String,
    #[serde(default)]
    tools: Vec<ToolYaml>,
}

#[derive(Debug, Deserialize, Default)]
struct ToolYaml {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    system: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    access: String,
    #[serde(default)]
    risk_level: String,
    #[serde(default)]
    sensitivity: String,
    #[serde(default)]
    required_permissions: Vec<String>,
    #[serde(default)]
    rate_limit_per_min: i64,
    #[serde(default)]
    log_retention_days: i64,
    #[serde(default)]
    timeout_ms: i64,
    #[serde(default)]
    max_retries: i64,
    #[serde(default)]
    approval_ttl_hours: i64,
    #[serde(default)]
    fallback: String,
    #[serde(default)]
    mask_fields: Vec<String>,
    #[serde(default)]
    input_schema: Value,
    #[serde(default)]
    output_schema: Value,
}

/// 동적 카탈로그 관리자.
pub struct Manager {
    path: Option<PathBuf>,
    reg: Arc<Registry>,
    inv: Option<Arc<Inventory>>,
    /// 이 관리자가 등록한 도구 이름 — 리로드 시 정확히 이것들만 걷어냅니다.
    owned: Mutex<HashSet<String>>,
}

impl std::fmt::Debug for Manager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("Manager").field("path", &self.path).finish() }
}

impl Manager {
    pub fn new(path: Option<PathBuf>, reg: Arc<Registry>, inv: Option<Arc<Inventory>>) -> Self {
        Self {
            path,
            reg,
            inv,
            owned: Mutex::new(HashSet::new()),
        }
    }

    pub fn path(&self) -> Option<&Path> { self.path.as_deref() }

    pub fn enabled(&self) -> bool { self.path.is_some() }

    pub fn owned_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.owned.lock().unwrap().iter().cloned().collect();
        v.sort();
        v
    }

    /// 카탈로그를 다시 읽어 적용합니다.
    ///
    /// 반환: `(added, removed)`.
    pub fn reload(&self) -> Result<(usize, usize)> {
        let Some(path) = &self.path else {
            return Ok((0, 0));
        };
        let tools = self.load_tools(path)?;

        // 이전에 등록한 것을 걷어냅니다 (내장 어댑터 도구는 건드리지 않습니다).
        let mut owned = self.owned.lock().unwrap();
        let mut removed = 0;
        for name in owned.iter() {
            if self.reg.unregister(name).is_ok() {
                removed += 1;
            }
        }
        owned.clear();

        let mut added = 0;
        for t in tools {
            let name = t.spec.name.clone();
            if let Err(e) = self.reg.replace(t) {
                // 실패하면 방금 넣은 것을 전부 되돌립니다 — 반쯤 적용된 카탈로그는 없습니다.
                for n in owned.iter() {
                    let _ = self.reg.unregister(n);
                }
                owned.clear();
                return Err(anyhow!("toolcatalog: register {name:?}: {e}"));
            }
            owned.insert(name);
            added += 1;
        }
        Ok((added, removed))
    }

    fn load_tools(&self, path: &Path) -> Result<Vec<Tool>> {
        if !path.exists() {
            return Ok(Vec::new()); // 파일이 없는 것은 오류가 아닙니다.
        }
        let data = std::fs::read_to_string(path)?;
        if data.trim().is_empty() {
            return Ok(Vec::new());
        }
        let doc: FileDoc = serde_norway::from_str(&data).map_err(|e| anyhow!("parse tool catalog {}: {e}", path.display()))?;

        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for (i, raw) in doc.tools.iter().enumerate() {
            if !seen.insert(raw.name.clone()) {
                bail!("duplicate tool name {:?} in catalog", raw.name);
            }
            let t = self.build_tool(raw).map_err(|e| anyhow!("tools[{i}] {:?}: {e}", raw.name))?;
            out.push(t);
        }
        Ok(out)
    }

    fn build_tool(&self, raw: &ToolYaml) -> Result<Tool> {
        if raw.name.is_empty() {
            bail!("name is required");
        }
        if raw.system.is_empty() {
            bail!("system is required");
        }
        let access = Access::parse(&raw.access)?;

        // base_url: 명시값 우선, 없으면 인벤토리의 rest/soap base_url.
        let mut base_url = raw.base_url.trim().to_string();
        if let Some(inv) = &self.inv {
            let sys = inv
                .lookup(&raw.system)
                .ok_or_else(|| anyhow!("system {:?} 이 인벤토리에 없습니다", raw.system))?;
            if base_url.is_empty() && sys.interface.is_network() {
                base_url = sys.base_url.clone();
            }
            // 인벤토리가 쓰기를 허용하지 않는 시스템에 쓰기 도구를 붙일 수 없습니다.
            if access == Access::Write && !sys.allows_write() {
                bail!("system {:?} 에 쓰기 capability 가 없습니다", raw.system);
            }
        }
        if base_url.is_empty() {
            bail!("base_url 이 없고 인벤토리에도 rest/soap base_url 이 없습니다");
        }

        let risk = if raw.risk_level.is_empty() {
            RiskLevel::L1
        } else {
            RiskLevel::parse(&raw.risk_level)?
        };

        let spec = Spec {
            name: raw.name.clone(),
            description: if raw.description.is_empty() {
                format!("remote tool {}", raw.name)
            } else {
                raw.description.clone()
            },
            system: raw.system.clone(),
            access,
            risk_level: risk,
            sensitivity: Sensitivity::parse(&raw.sensitivity),
            required_permissions: raw.required_permissions.clone(),
            approval_required: false,
            approval_ttl: if raw.approval_ttl_hours > 0 {
                Duration::from_secs(raw.approval_ttl_hours as u64 * 3600)
            } else {
                Duration::ZERO
            },
            rate_limit_per_min: if raw.rate_limit_per_min <= 0 { 60 } else { raw.rate_limit_per_min },
            timeout_ms: raw.timeout_ms,
            max_retries: raw.max_retries,
            log_retention_days: if raw.log_retention_days <= 0 { 90 } else { raw.log_retention_days },
            mask_fields: raw.mask_fields.clone(),
            fallback: if raw.fallback.is_empty() {
                "담당 시스템에 직접 요청하세요.".to_string()
            } else {
                raw.fallback.clone()
            },
            input_schema: raw.input_schema.clone(),
            output_schema: raw.output_schema.clone(),
        };

        let handler = adapter::remote_handler(&base_url, &raw.name)?;
        Ok(Tool {
            spec,
            handler,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{AuthMethod,
                           Capability,
                           FailureImpact,
                           Interface,
                           Realtime,
                           Sensitivity as InvSens,
                           System};

    fn inv() -> Arc<Inventory> {
        Arc::new(Inventory::new(vec![
            System {
                name: "ticket".into(),
                display_name: "티켓".into(),
                interface: Interface::Rest,
                base_url: "https://ticket.example".into(),
                data_sensitivity: vec![InvSens::General],
                capabilities: vec![Capability::Read, Capability::Create],
                owner_team: "고객지원팀".into(),
                contact: "help@example.com".into(),
                auth_method: AuthMethod::Sso,
                failure_impact: FailureImpact::Low,
                realtime: Realtime::Realtime,
            },
            System {
                name: "crm".into(),
                display_name: "CRM".into(),
                interface: Interface::Rest,
                base_url: "https://crm.example".into(),
                data_sensitivity: vec![InvSens::Personal],
                capabilities: vec![Capability::Read], // 읽기 전용
                owner_team: "영업지원팀".into(),
                contact: "sales@example.com".into(),
                auth_method: AuthMethod::Sso,
                failure_impact: FailureImpact::Medium,
                realtime: Realtime::Realtime,
            },
        ]))
    }

    fn write_catalog(body: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tools-dynamic.yaml");
        std::fs::write(&path, body).unwrap();
        (dir, path)
    }

    const VALID: &str = r#"
version: "1"
tools:
  - name: remote_lookup
    description: 원격 조회
    system: ticket
    access: read
    risk_level: L1
    required_permissions: [ticket.read]
    input_schema: {type: object, properties: {}, additionalProperties: false}
    output_schema: {type: object, properties: {}, additionalProperties: false}
"#;

    #[test]
    fn registers_tools_from_yaml() {
        let (_d, path) = write_catalog(VALID);
        let reg = Arc::new(Registry::new());
        let m = Manager::new(Some(path), reg.clone(), Some(inv()));

        let (added, removed) = m.reload().unwrap();
        assert_eq!((added, removed), (1, 0));
        assert!(reg.lookup("remote_lookup").is_some());
        assert_eq!(m.owned_names(), vec!["remote_lookup"]);
    }

    #[test]
    fn reload_replaces_previously_owned_tools() {
        let (_d, path) = write_catalog(VALID);
        let reg = Arc::new(Registry::new());
        let m = Manager::new(Some(path.clone()), reg.clone(), Some(inv()));
        m.reload().unwrap();

        std::fs::write(&path, "version: \"1\"\ntools: []\n").unwrap();
        let (added, removed) = m.reload().unwrap();
        assert_eq!((added, removed), (0, 1));
        assert!(reg.lookup("remote_lookup").is_none());
    }

    #[test]
    fn missing_file_is_not_an_error() {
        let reg = Arc::new(Registry::new());
        let m = Manager::new(Some(PathBuf::from("/nonexistent.yaml")), reg, Some(inv()));
        assert_eq!(m.reload().unwrap(), (0, 0));
    }

    #[test]
    fn base_url_falls_back_to_inventory() {
        let (_d, path) = write_catalog(VALID);
        let reg = Arc::new(Registry::new());
        let m = Manager::new(Some(path), reg.clone(), Some(inv()));
        // base_url 을 안 적었지만 인벤토리의 ticket.base_url 을 씁니다.
        assert!(m.reload().is_ok());
    }

    #[test]
    fn rejects_write_tool_on_read_only_system() {
        let body = r#"
tools:
  - name: bad_write
    system: crm
    access: write
    risk_level: L3
    required_permissions: [crm.customer.read]
    input_schema: {type: object, properties: {}, additionalProperties: false}
    output_schema: {type: object, properties: {}, additionalProperties: false}
"#;
        let (_d, path) = write_catalog(body);
        let m = Manager::new(Some(path), Arc::new(Registry::new()), Some(inv()));
        let err = m.reload().unwrap_err().to_string();
        assert!(err.contains("쓰기 capability"));
    }

    #[test]
    fn rejects_unknown_system() {
        let body = r#"
tools:
  - name: t
    system: nope
    required_permissions: [x]
    input_schema: {type: object}
    output_schema: {type: object}
"#;
        let (_d, path) = write_catalog(body);
        let m = Manager::new(Some(path), Arc::new(Registry::new()), Some(inv()));
        assert!(m.reload().unwrap_err().to_string().contains("인벤토리에 없습니다"));
    }

    #[test]
    fn rejects_duplicate_names() {
        let body = r#"
tools:
  - name: dup
    system: ticket
    required_permissions: [ticket.read]
    input_schema: {type: object, properties: {}, additionalProperties: false}
    output_schema: {type: object, properties: {}, additionalProperties: false}
  - name: dup
    system: ticket
    required_permissions: [ticket.read]
    input_schema: {type: object, properties: {}, additionalProperties: false}
    output_schema: {type: object, properties: {}, additionalProperties: false}
"#;
        let (_d, path) = write_catalog(body);
        let m = Manager::new(Some(path), Arc::new(Registry::new()), Some(inv()));
        assert!(m.reload().unwrap_err().to_string().contains("duplicate"));
    }

    #[test]
    fn dynamic_tools_still_face_registry_validation() {
        // 스키마 없는 도구는 YAML 로 선언해도 거부됩니다.
        let body = r#"
tools:
  - name: no_schema
    system: ticket
    required_permissions: [ticket.read]
"#;
        let (_d, path) = write_catalog(body);
        let m = Manager::new(Some(path), Arc::new(Registry::new()), Some(inv()));
        assert!(m.reload().is_err());
    }

    #[test]
    fn failed_reload_leaves_no_half_applied_tools() {
        let body = r#"
tools:
  - name: good
    system: ticket
    required_permissions: [ticket.read]
    input_schema: {type: object, properties: {}, additionalProperties: false}
    output_schema: {type: object, properties: {}, additionalProperties: false}
  - name: bad
    system: ticket
    required_permissions: [ticket.read]
"#;
        let (_d, path) = write_catalog(body);
        let reg = Arc::new(Registry::new());
        let m = Manager::new(Some(path), reg.clone(), Some(inv()));
        assert!(m.reload().is_err());
        // 앞의 good 도 남지 않아야 합니다.
        assert!(reg.lookup("good").is_none());
        assert!(m.owned_names().is_empty());
    }
}
