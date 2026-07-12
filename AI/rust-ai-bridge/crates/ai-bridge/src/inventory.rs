//! 레거시 시스템 인벤토리 (`systems.yaml`).
//!
//! "AI가 어디까지 해도 되는지"를 통제하려면 먼저 무엇이 있는지 알아야 합니다.
//! **이 파일은 문서가 아니라 코드가 읽는 설정입니다.**
//!
//! - 인벤토리에 없는 시스템의 도구는 등록되지 않습니다.
//! - `capabilities` 에 쓰기 기능이 없으면 그 시스템에 쓰기 도구를 붙일 수
//!   없습니다.
//!
//! 이 두 검사가 인벤토리를 실제 통제 수단으로 만듭니다. 인벤토리와 코드가
//! 어긋나면 게이트웨이가 기동하지 않습니다.

use anyhow::{Result,
             anyhow,
             bail};
use serde::{Deserialize,
            Serialize};
use std::{collections::{HashMap,
                        HashSet},
          path::Path,
          sync::RwLock};

macro_rules! str_enum {
    ($name:ident, $($variant:ident => $s:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $(#[serde(rename = $s)] $variant),+
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(match self { $($name::$variant => $s),+ })
            }
        }
    };
}

str_enum!(Interface,
    Rest => "rest",
    Soap => "soap",
    Db => "db",
    File => "file",
    Batch => "batch",
    Rpa => "rpa",
    Mainframe => "mainframe",
    Memory => "memory",
);

impl Interface {
    /// `base_url` 이 의미를 갖는 전송인지.
    pub fn is_network(self) -> bool { matches!(self, Interface::Rest | Interface::Soap) }
}

str_enum!(Capability,
    Read => "read",
    Create => "create",
    Update => "update",
    Delete => "delete",
    Approve => "approve",
);

impl Capability {
    pub fn is_write(self) -> bool { self != Capability::Read }
}

str_enum!(FailureImpact, Low => "low", Medium => "medium", High => "high");
str_enum!(Realtime, Realtime => "realtime", NearRealtime => "near_realtime", Batch => "batch");
str_enum!(AuthMethod,
    Sso => "sso",
    ApiKey => "api_key",
    DbAccount => "db_account",
    Vpn => "vpn",
    None => "none",
);
str_enum!(Sensitivity,
    Personal => "personal",
    Financial => "financial",
    TradeSecret => "trade_secret",
    General => "general",
);

/// 레거시 시스템 하나.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct System {
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    pub interface: Interface,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub data_sensitivity: Vec<Sensitivity>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub owner_team: String,
    #[serde(default)]
    pub contact: String,
    pub auth_method: AuthMethod,
    pub failure_impact: FailureImpact,
    pub realtime: Realtime,
}

impl System {
    pub fn allows_write(&self) -> bool { self.capabilities.iter().any(|c| c.is_write()) }

    /// 권한이 없을 때 **어디에 요청해야 하는지** 안내합니다.
    ///
    /// "담당 부서에 문의하세요"가 아니라
    /// "영업지원팀(sales-support@example.com)에 접근 요청을 제출하세요"가
    /// 되어야 사용자가 다음 행동을 할 수 있습니다.
    pub fn access_request_path(&self) -> String {
        match (self.owner_team.is_empty(), self.contact.is_empty()) {
            | (false, false) => format!("{}({})에 접근 요청을 제출하세요", self.owner_team, self.contact),
            | (false, true) => format!("{}에 접근 요청을 제출하세요", self.owner_team),
            | _ => "담당 부서에 접근 요청을 제출하세요".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct SystemsFile {
    #[serde(default)]
    systems: Vec<System>,
}

/// 인벤토리 (핫 리로드 + 1단계 롤백).
#[derive(Debug, Default)]
pub struct Inventory {
    inner: RwLock<InventoryState>,
}

#[derive(Debug, Default)]
struct InventoryState {
    systems: HashMap<String, System>,
    previous: Option<HashMap<String, System>>,
}

fn index(systems: Vec<System>) -> HashMap<String, System> { systems.into_iter().map(|s| (s.name.clone(), s)).collect() }

impl Inventory {
    /// 검증 없이 만듭니다 (테스트용).
    pub fn new(systems: Vec<System>) -> Self {
        Self {
            inner: RwLock::new(InventoryState {
                systems: index(systems),
                previous: None,
            }),
        }
    }

    /// 파일을 읽고 검증합니다.
    pub fn load(path: &Path) -> Result<Self> {
        let systems = read_file(path)?;
        validate(&systems)?;
        Ok(Self::new(systems))
    }

    /// 파일을 다시 읽습니다. **검증 실패 시 활성 상태는 그대로입니다.**
    pub fn reload(&self, path: &Path) -> Result<()> {
        let systems = read_file(path)?;
        validate(&systems)?;
        self.replace(systems)
    }

    pub fn replace(&self, systems: Vec<System>) -> Result<()> {
        validate(&systems)?;
        let mut st = self.inner.write().unwrap();
        st.previous = Some(st.systems.clone());
        st.systems = index(systems);
        Ok(())
    }

    pub fn rollback(&self) -> Result<()> {
        let mut st = self.inner.write().unwrap();
        let Some(prev) = st.previous.take() else {
            bail!("inventory: no previous snapshot");
        };
        let current = std::mem::replace(&mut st.systems, prev);
        st.previous = Some(current);
        Ok(())
    }

    pub fn lookup(&self, name: &str) -> Option<System> { self.inner.read().unwrap().systems.get(name).cloned() }

    /// 시스템을 이름순으로 돌려줍니다.
    pub fn systems(&self) -> Vec<System> {
        let st = self.inner.read().unwrap();
        let mut v: Vec<System> = st.systems.values().cloned().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    pub fn len(&self) -> usize { self.inner.read().unwrap().systems.len() }

    pub fn is_empty(&self) -> bool { self.len() == 0 }

    pub fn access_request_path(&self, system: &str) -> String {
        self.lookup(system)
            .map(|s| s.access_request_path())
            .unwrap_or_else(|| "담당 부서에 접근 요청을 제출하세요".to_string())
    }
}

fn read_file(path: &Path) -> Result<Vec<System>> {
    let data = std::fs::read_to_string(path).map_err(|e| anyhow!("read systems file {}: {e}", path.display()))?;
    let f: SystemsFile = serde_norway::from_str(&data).map_err(|e| anyhow!("parse systems file {}: {e}", path.display()))?;
    Ok(f.systems)
}

/// 인벤토리를 검증합니다.
pub fn validate(systems: &[System]) -> Result<()> {
    if systems.is_empty() {
        bail!("at least one system is required");
    }
    let mut seen = HashSet::new();
    for (i, s) in systems.iter().enumerate() {
        if s.name.is_empty() {
            bail!("system[{i}]: name is required");
        }
        if !seen.insert(&s.name) {
            bail!("system {:?}: duplicate system name", s.name);
        }
        // memory/db/file 시스템에 base_url 이 있으면 설정이 잘못된 것입니다.
        if !s.interface.is_network() && !s.base_url.is_empty() {
            bail!("system {:?}: base_url is only meaningful for a network interface (rest, soap)", s.name);
        }
        if s.capabilities.is_empty() {
            bail!("system {:?}: at least one capability is required", s.name);
        }
        // 권한 요청 경로 안내에 필요합니다.
        if s.owner_team.is_empty() {
            bail!("system {:?}: owner_team is required (권한 요청 경로 안내에 필요)", s.name);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sys(name: &str) -> System {
        System {
            name: name.into(),
            display_name: name.into(),
            interface: Interface::Memory,
            base_url: String::new(),
            data_sensitivity: vec![Sensitivity::General],
            capabilities: vec![Capability::Read],
            owner_team: "IT운영팀".into(),
            contact: "it-ops@example.com".into(),
            auth_method: AuthMethod::Sso,
            failure_impact: FailureImpact::Low,
            realtime: Realtime::Realtime,
        }
    }

    #[test]
    fn loads_the_real_systems_yaml() {
        // 실제 설정 파일이 검증을 통과해야 합니다.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/systems.yaml");
        let inv = Inventory::load(&path).expect("config/systems.yaml should be valid");
        assert_eq!(inv.len(), 6);
        assert!(inv.lookup("erp").is_some());
        assert!(inv.lookup("refund").unwrap().allows_write());
        assert!(!inv.lookup("crm").unwrap().allows_write());
    }

    #[test]
    fn rejects_duplicate_names() {
        assert!(validate(&[sys("a"), sys("a")]).is_err());
    }

    #[test]
    fn rejects_base_url_on_non_network_interface() {
        let mut s = sys("a");
        s.base_url = "https://x".into();
        assert!(validate(&[s]).is_err());
    }

    #[test]
    fn allows_base_url_on_rest() {
        let mut s = sys("a");
        s.interface = Interface::Rest;
        s.base_url = "https://x".into();
        assert!(validate(&[s]).is_ok());
    }

    #[test]
    fn rejects_missing_owner_team() {
        let mut s = sys("a");
        s.owner_team = String::new();
        assert!(validate(&[s]).is_err());
    }

    #[test]
    fn rejects_empty_capabilities_and_empty_inventory() {
        let mut s = sys("a");
        s.capabilities = vec![];
        assert!(validate(&[s]).is_err());
        assert!(validate(&[]).is_err());
    }

    #[test]
    fn write_capability_detection() {
        let mut s = sys("a");
        assert!(!s.allows_write());
        s.capabilities.push(Capability::Create);
        assert!(s.allows_write());
    }

    #[test]
    fn access_request_path_names_the_team() {
        assert_eq!(sys("a").access_request_path(), "IT운영팀(it-ops@example.com)에 접근 요청을 제출하세요");
    }

    #[test]
    fn unknown_system_falls_back_to_generic_guidance() {
        let inv = Inventory::new(vec![sys("a")]);
        assert_eq!(inv.access_request_path("nope"), "담당 부서에 접근 요청을 제출하세요");
    }

    #[test]
    fn rollback_restores_previous_snapshot() {
        let inv = Inventory::new(vec![sys("a")]);
        inv.replace(vec![sys("a"), sys("b")]).unwrap();
        assert_eq!(inv.len(), 2);
        inv.rollback().unwrap();
        assert_eq!(inv.len(), 1);
    }

    #[test]
    fn rollback_without_snapshot_errors() {
        assert!(Inventory::new(vec![sys("a")]).rollback().is_err());
    }
}
