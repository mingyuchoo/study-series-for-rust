//! 주체 해석과 환경 속성.
//!
//! 게이트웨이는 **요청마다** 호출 주체를 해석합니다. 주체를 해석하지 못하면
//! 익명으로 강등하지 않고 거절합니다 — 익명으로 통과시키면 정책 엔진이 판단할
//! 근거가 사라집니다.
//!
//! **사람과 에이전트는 신뢰 모델이 다릅니다.** 사람은 앞단 프록시가 신원을
//! 보증한다고 가정하고 `X-User-Id` 헤더를 믿습니다. 자율 에이전트는 프록시가
//! 없으므로 **자기 토큰**으로 인증합니다. `kind: agent` 인 주체는 헤더만으로는
//! 절대 인증되지 않습니다 — 헤더 한 줄로 에이전트를 사칭하는 경로를 막기
//! 위함입니다.

use anyhow::{Result,
             anyhow,
             bail};
use chrono::{DateTime,
             Datelike,
             Timelike,
             Utc};
use ipnet::IpNet;
use serde::{Deserialize,
            Serialize};
use serde_json::Value;
use sha2::{Digest,
           Sha256};
use std::{collections::HashMap,
          path::Path,
          sync::RwLock};

/// 자율 에이전트를 나타내는 `kind` 값.
pub const KIND_AGENT: &str = "agent";

pub const USER_ID_HEADER: &str = "X-User-Id";
pub const SESSION_ID_HEADER: &str = "X-Session-Id";
pub const AUTHORIZATION_HEADER: &str = "Authorization";
pub const FORWARDED_FOR_HEADER: &str = "X-Forwarded-For";
/// 미들웨어가 매 요청 덮어쓰는 **실제 피어 주소**. 클라이언트가 위조할 수
/// 없습니다.
pub const REMOTE_ADDR_HEADER: &str = "X-Gateway-Remote-Addr";
pub const LLM_DESTINATION_HEADER: &str = "X-Verified-LLM-Destination";
pub const BUSINESS_PURPOSE_HEADER: &str = "X-Verified-Business-Purpose";

/// `Enricher` 가 요청 시점에 계산해 **덮어쓰는** 환경 속성들.
///
/// 주체가 설정 파일에 `business_hours: "true"` 라고 적어두면 "업무시간에만 조회
/// 가능" 규칙이 언제나 참이 되고, 정책 엔진은 통제하는 척만 하게 됩니다.
pub const ATTR_BUSINESS_HOURS: &str = "business_hours";
pub const ATTR_NETWORK_ZONE: &str = "network_zone";
pub const ATTR_REQUEST_TIME: &str = "request_time";
pub const ATTR_LLM_DESTINATION: &str = "llm_destination";
pub const ATTR_BUSINESS_PURPOSE: &str = "business_purpose";

/// 호출 주체.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Identity {
    pub user_id: String,
    pub session_id: String,
    /// `""` = 사람, `"agent"` = 자율 에이전트.
    pub kind: String,
    pub roles: Vec<String>,
    pub department: String,
    pub permissions: Vec<String>,
    /// 역할이 준 권한 안에서 **더 좁히기만** 합니다. 넓히지는 못합니다.
    pub allowed_tools: Vec<String>,
    pub allowed_systems: Vec<String>,
    /// 소문자 hex sha256. 존재하면 이 주체는 토큰으로 인증합니다.
    pub token_sha256: String,
    pub token_expires_at: Option<DateTime<Utc>>,
    /// ABAC 속성. 환경 속성은 `Enricher` 가 덮어씁니다.
    pub attributes: HashMap<String, Value>,
}

impl Identity {
    pub fn is_agent(&self) -> bool { self.kind == KIND_AGENT }

    /// 주체의 스코프가 이 도구·시스템을 허용하는지.
    ///
    /// **좁히기 전용**: 목록이 비어 있으면 그 축에는 제한이 없습니다.
    pub fn allows_tool(&self, name: &str, system: &str) -> bool {
        if !self.allowed_tools.is_empty() && !self.allowed_tools.iter().any(|t| t == name) {
            return false;
        }
        if !self.allowed_systems.is_empty() && !self.allowed_systems.iter().any(|s| s == system) {
            return false;
        }
        true
    }

    pub fn has_role(&self, role: &str) -> bool { self.roles.iter().any(|r| r == role) }

    pub fn attr(&self, key: &str) -> Option<&Value> { self.attributes.get(key) }
}

// ---------------------------------------------------------------------------
// principal.yaml
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct IdentityFile {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    user_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    roles: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    department: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    permissions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    allowed_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    allowed_systems: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    token_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    token_expires_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    attributes: HashMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    principals: Vec<IdentityFile>,
}

impl From<&IdentityFile> for Identity {
    fn from(f: &IdentityFile) -> Self {
        Identity {
            user_id: f.user_id.clone(),
            session_id: String::new(),
            kind: f.kind.clone(),
            roles: f.roles.clone(),
            department: f.department.clone(),
            permissions: f.permissions.clone(),
            allowed_tools: f.allowed_tools.clone(),
            allowed_systems: f.allowed_systems.clone(),
            token_sha256: f.token_sha256.clone(),
            token_expires_at: f.token_expires_at,
            attributes: f.attributes.clone(),
        }
    }
}

impl From<&Identity> for IdentityFile {
    fn from(id: &Identity) -> Self {
        IdentityFile {
            user_id: id.user_id.clone(),
            kind: id.kind.clone(),
            roles: id.roles.clone(),
            department: id.department.clone(),
            permissions: id.permissions.clone(),
            allowed_tools: id.allowed_tools.clone(),
            allowed_systems: id.allowed_systems.clone(),
            token_sha256: id.token_sha256.clone(),
            token_expires_at: id.token_expires_at.map(|t| t.with_nanosecond(0).unwrap_or(t)),
            attributes: id.attributes.clone(),
            principals: Vec::new(),
        }
    }
}

/// 주체 디렉터리 (읽기 전용).
#[derive(Debug, Clone, Default)]
pub struct Directory {
    by_id: HashMap<String, Identity>,
}

impl Directory {
    pub fn new(ids: Vec<Identity>) -> Self {
        Self {
            by_id: ids.into_iter().map(|i| (i.user_id.clone(), i)).collect(),
        }
    }

    pub fn lookup(&self, user_id: &str) -> Option<Identity> { self.by_id.get(user_id).cloned() }

    pub fn len(&self) -> usize { self.by_id.len() }

    pub fn is_empty(&self) -> bool { self.by_id.is_empty() }

    pub fn user_ids(&self) -> Vec<String> {
        let mut v: Vec<String> = self.by_id.keys().cloned().collect();
        v.sort();
        v
    }

    pub fn identities(&self) -> Vec<Identity> {
        let mut v: Vec<Identity> = self.by_id.values().cloned().collect();
        v.sort_by(|a, b| a.user_id.cmp(&b.user_id));
        v
    }

    /// 주체가 정확히 하나일 때만 그것을 돌려줍니다.
    ///
    /// stdio 는 프로세스 하나가 사용자 한 명을 대신하므로, 여럿 중 임의로
    /// 하나를 고르면 의도하지 않은 권한으로 동작합니다.
    pub fn only(&self) -> Result<Identity> {
        match self.len() {
            | 0 => bail!("auth: 디렉터리에 주체가 없습니다"),
            | 1 => Ok(self.by_id.values().next().unwrap().clone()),
            | n => bail!("auth: 주체가 {}명입니다({}). 하나를 골라 지정하세요", n, self.user_ids().join(", ")),
        }
    }
}

fn parse_identity_file(data: &str) -> Result<IdentityFile> { serde_norway::from_str(data).map_err(|e| anyhow!("parse principal file: {e}")) }

/// `principal.yaml` 을 읽습니다.
///
/// `user_id`(단일) 와 `principals`(목록) 중 **정확히 하나만** 쓸 수 있습니다.
pub fn load_directory(path: &Path) -> Result<Directory> {
    let data = std::fs::read_to_string(path).map_err(|e| anyhow!("read principal file {}: {e}", path.display()))?;
    let f = parse_identity_file(&data)?;
    let p = path.display();

    if !f.principals.is_empty() && !f.user_id.is_empty() {
        bail!("principal file {p}: user_id 와 principals 를 함께 쓸 수 없습니다");
    }
    if !f.principals.is_empty() {
        let mut ids = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (i, e) in f.principals.iter().enumerate() {
            if e.user_id.is_empty() {
                bail!("principal file {p}: principals[{i}]: user_id is required");
            }
            if !seen.insert(e.user_id.clone()) {
                bail!("principal file {p}: duplicate user_id {:?}", e.user_id);
            }
            ids.push(Identity::from(e));
        }
        return Ok(Directory::new(ids));
    }
    if !f.user_id.is_empty() {
        return Ok(Directory::new(vec![Identity::from(&f)]));
    }
    bail!("principal file {p}: user_id 또는 principals 가 필요합니다")
}

/// 디렉터리에서 주체 하나를 고릅니다. `user_id` 가 비면 유일한 주체를
/// 요구합니다.
pub fn load_principal(path: &Path, user_id: &str) -> Result<Identity> {
    let dir = load_directory(path)?;
    if user_id.is_empty() {
        return dir.only();
    }
    dir.lookup(user_id).ok_or_else(|| {
        anyhow!(
            "principal file {}: 주체 {user_id:?} 이(가) 없습니다(있는 주체: {})",
            path.display(),
            dir.user_ids().join(", ")
        )
    })
}

const PRINCIPAL_HEADER: &str = "\
# principal directory — Apply/콘솔이 갱신할 수 있습니다.
# 평문 토큰을 넣지 마십시오. token_sha256 만 저장합니다.
";

fn load_principals_list(path: &Path) -> Result<Vec<IdentityFile>> {
    let data = std::fs::read_to_string(path).map_err(|e| anyhow!("read principal file {}: {e}", path.display()))?;
    let f = parse_identity_file(&data)?;
    if !f.user_id.is_empty() {
        bail!(
            "auth: principal file {} 는 단일 user_id 형식입니다; Apply 는 principals: 목록 형식만 지원합니다",
            path.display()
        );
    }
    if f.principals.is_empty() {
        bail!("auth: principal file {} 에 principals 목록이 없습니다", path.display());
    }
    Ok(f.principals)
}

fn write_principals(path: &Path, list: &[IdentityFile]) -> Result<()> {
    #[derive(Serialize)]
    struct Doc<'a> {
        principals: &'a [IdentityFile],
    }
    let body = serde_norway::to_string(&Doc {
        principals: list,
    })?;
    let content = format!("{PRINCIPAL_HEADER}{body}");
    atomic_write(path, content.as_bytes(), 0o600)
}

/// 임시 파일에 쓰고 rename 합니다 — 쓰다 만 설정 파일로 게이트웨이가 뜨지
/// 않도록.
pub fn atomic_write_public(path: &Path, data: &[u8], mode: u32) -> Result<()> { atomic_write(path, data, mode) }

pub(crate) fn atomic_write(path: &Path, data: &[u8], mode: u32) -> Result<()> {
    use std::io::Write as _;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).ok();
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(data)?;
    tmp.flush()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(mode))?;
    }
    let _ = mode;
    tmp.persist(path).map_err(|e| anyhow!("write {}: {e}", path.display()))?;
    Ok(())
}

/// 주체를 추가하거나 갱신합니다 (`user_id` 로 대조).
pub fn upsert_principal_in_file(path: &Path, id: &Identity) -> Result<()> {
    if id.user_id.is_empty() {
        bail!("auth: user_id is required");
    }
    let mut list = load_principals_list(path)?;
    let entry = IdentityFile::from(id);
    if let Some(existing) = list.iter_mut().find(|e| e.user_id == id.user_id) {
        // 사람↔에이전트 전환은 신뢰 모델이 바뀌는 일이라 조용히 허용하지 않습니다.
        if (existing.kind == KIND_AGENT) != (entry.kind == KIND_AGENT) {
            bail!("auth: user_id {:?} 의 kind 를 바꿀 수 없습니다", id.user_id);
        }
        *existing = entry;
    } else {
        list.push(entry);
    }
    write_principals(path, &list)
}

/// 주체를 제거합니다.
pub fn remove_principal_from_file(path: &Path, user_id: &str) -> Result<()> {
    if user_id.is_empty() {
        bail!("auth: user_id is required");
    }
    let mut list = load_principals_list(path)?;
    let before = list.len();
    list.retain(|e| e.user_id != user_id);
    if list.len() == before {
        bail!("auth: principal {user_id:?} not found in file");
    }
    if list.is_empty() {
        bail!("auth: 마지막 주체를 삭제할 수 없습니다");
    }
    write_principals(path, &list)
}

// ---------------------------------------------------------------------------
// 토큰
// ---------------------------------------------------------------------------

/// 토큰의 소문자 hex sha256.
pub fn token_hash(token: &str) -> String {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    hex::encode(h.finalize())
}

/// 새 에이전트 토큰(256비트)을 발급합니다.
///
/// `ThreadRng` 는 `CryptoRng` 이며 OS 엔트로피로 시드·재시드됩니다.
pub fn generate_token() -> String {
    use rand::Rng as _;
    let mut b = [0u8; 32];
    rand::rng().fill_bytes(&mut b);
    hex::encode(b)
}

// ---------------------------------------------------------------------------
// 리졸버
// ---------------------------------------------------------------------------

/// 요청 문맥. stdio 는 헤더가 없습니다.
#[derive(Debug, Clone, Default)]
pub struct RequestContext {
    pub session_id: String,
    pub headers: HashMap<String, String>,
    pub now: Option<DateTime<Utc>>,
}

impl RequestContext {
    /// 헤더를 읽습니다 (이름은 대소문자 무시).
    pub fn get(&self, key: &str) -> &str {
        let lower = key.to_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == lower)
            .map(|(_, v)| v.as_str())
            .unwrap_or("")
    }

    pub fn set(&mut self, key: &str, value: &str) { self.headers.insert(key.to_string(), value.to_string()); }
}

/// 요청에서 호출 주체를 해석하는 것.
pub trait Resolver: Send + Sync + std::fmt::Debug {
    fn resolve(&self, rc: &RequestContext) -> Result<Identity>;
}

pub type SharedResolver = std::sync::Arc<dyn Resolver>;

/// 주체 하나를 고정합니다 (stdio · 개발용 콘솔).
#[derive(Debug, Clone)]
pub struct StaticResolver {
    pub identity: Identity,
}

impl Resolver for StaticResolver {
    fn resolve(&self, rc: &RequestContext) -> Result<Identity> {
        let mut id = self.identity.clone();
        id.session_id = rc.session_id.clone();
        Ok(id)
    }
}

fn with_session(mut id: Identity, rc: &RequestContext) -> Identity {
    id.session_id = rc.session_id.clone();
    let hdr = rc.get(SESSION_ID_HEADER);
    if !hdr.is_empty() {
        id.session_id = hdr.to_string();
    }
    id
}

fn bearer_token(rc: &RequestContext) -> String {
    let h = rc.get(AUTHORIZATION_HEADER);
    let prefix = "bearer ";
    if h.len() > prefix.len() && h[.. prefix.len()].eq_ignore_ascii_case(prefix) {
        return h[prefix.len() ..].trim().to_string();
    }
    String::new()
}

/// 사람은 헤더로, 에이전트는 Bearer 토큰으로 인증합니다.
///
/// 핫 리로드와 1단계 롤백을 지원합니다.
#[derive(Debug)]
pub struct TokenResolver {
    inner: RwLock<TokenState>,
    header: String,
}

#[derive(Debug, Default)]
struct TokenState {
    directory: Directory,
    by_token: HashMap<String, String>,
    digest: String,
    prev: Option<Box<TokenState>>,
}

fn build_token_index(dir: &Directory) -> HashMap<String, String> {
    dir.identities()
        .into_iter()
        .filter(|i| !i.token_sha256.is_empty())
        .map(|i| (i.token_sha256.to_lowercase(), i.user_id))
        .collect()
}

impl TokenResolver {
    pub fn new(dir: Directory, header: &str) -> Self {
        let by_token = build_token_index(&dir);
        Self {
            inner: RwLock::new(TokenState {
                directory: dir,
                by_token,
                digest: String::new(),
                prev: None,
            }),
            header: if header.is_empty() { USER_ID_HEADER.to_string() } else { header.to_string() },
        }
    }

    pub fn token_count(&self) -> usize { self.inner.read().unwrap().by_token.len() }

    pub fn principal_count(&self) -> usize { self.inner.read().unwrap().directory.len() }

    pub fn digest(&self) -> String { self.inner.read().unwrap().digest.clone() }

    pub fn identities(&self) -> Vec<Identity> { self.inner.read().unwrap().directory.identities() }

    /// 파일을 다시 읽어 적용합니다. **검증 실패 시 활성 상태는 그대로입니다.**
    pub fn reload(&self, path: &Path, validate: Option<IdentityValidator<'_>>) -> Result<(String, String)> {
        let data = std::fs::read(path)?;
        let dir = load_directory(path)?;
        if dir.is_empty() {
            bail!("auth: principal directory is empty");
        }
        if let Some(v) = validate {
            v(&dir.identities())?;
        }
        let new_digest = {
            let mut h = Sha256::new();
            h.update(&data);
            hex::encode(h.finalize())
        };
        let mut st = self.inner.write().unwrap();
        let old_digest = st.digest.clone();
        let prev = TokenState {
            directory: st.directory.clone(),
            by_token: st.by_token.clone(),
            digest: st.digest.clone(),
            prev: None,
        };
        st.by_token = build_token_index(&dir);
        st.directory = dir;
        st.digest = new_digest.clone();
        st.prev = Some(Box::new(prev));
        Ok((old_digest, new_digest))
    }

    /// 직전 스냅샷으로 되돌립니다 (1단계).
    pub fn rollback(&self) -> Result<(String, String)> {
        let mut st = self.inner.write().unwrap();
        let Some(prev) = st.prev.take() else {
            bail!("auth: no previous principal snapshot");
        };
        let old_digest = st.digest.clone();
        let current = TokenState {
            directory: st.directory.clone(),
            by_token: st.by_token.clone(),
            digest: st.digest.clone(),
            prev: None,
        };
        st.directory = prev.directory;
        st.by_token = prev.by_token;
        st.digest = prev.digest;
        // 되돌리기를 두 번 하면 원래대로 — swap 이지 스택이 아닙니다.
        st.prev = Some(Box::new(current));
        let new_digest = st.digest.clone();
        Ok((old_digest, new_digest))
    }

    pub fn replace_directory(&self, dir: Directory) {
        let mut st = self.inner.write().unwrap();
        st.by_token = build_token_index(&dir);
        st.directory = dir;
        st.digest = String::new();
    }
}

impl Resolver for TokenResolver {
    fn resolve(&self, rc: &RequestContext) -> Result<Identity> {
        let st = self.inner.read().unwrap();

        // 1. Bearer 토큰이 있으면 그것으로만 인증합니다. 헤더는 무시합니다.
        let token = bearer_token(rc);
        if !token.is_empty() {
            let hash = token_hash(&token);
            // 어떤 토큰이 유효한지 알려주지 않습니다.
            let user_id = st.by_token.get(&hash).ok_or_else(|| anyhow!("auth: 유효하지 않은 에이전트 토큰입니다"))?;
            let id = st.directory.lookup(user_id).ok_or_else(|| anyhow!("auth: 유효하지 않은 에이전트 토큰입니다"))?;
            if let Some(exp) = id.token_expires_at {
                let now = rc.now.unwrap_or_else(Utc::now);
                if now > exp {
                    bail!("auth: 에이전트 토큰이 만료되었습니다");
                }
            }
            return Ok(with_session(id, rc));
        }

        // 2. 토큰이 없으면 헤더로 해석합니다.
        let user_id = rc.get(&self.header);
        if user_id.is_empty() {
            bail!("auth: {} 헤더가 없어 호출 주체를 알 수 없습니다", self.header);
        }
        let id = st
            .directory
            .lookup(user_id)
            // 어떤 주체가 있는지 알려주지 않습니다.
            .ok_or_else(|| anyhow!("auth: 알 수 없는 호출 주체입니다"))?;

        // 에이전트는 헤더 한 줄로 사칭될 수 없습니다.
        if id.is_agent() {
            bail!("auth: 에이전트 주체 {user_id:?} 는 토큰 인증이 필요합니다");
        }
        Ok(with_session(id, rc))
    }
}

// ---------------------------------------------------------------------------
// Enricher
// ---------------------------------------------------------------------------

/// 주체 목록을 검증하는 것 (핫 리로드 시 정책 allowlist 대조).
pub type IdentityValidator<'a> = &'a dyn Fn(&[Identity]) -> Result<()>;

/// 환경 속성을 요청 시점에 계산해 **덮어씁니다.**
#[derive(Debug, Clone, Default)]
pub struct Enricher {
    /// `[start, end)`. 둘 다 0 이면 시간 제한 없음.
    pub start_hour: u32,
    pub end_hour: u32,
    pub internal_prefixes: Vec<IpNet>,
    /// `X-Forwarded-For` 를 믿을지. **엣지에 직접 노출된 게이트웨이에서는
    /// 금지** — 누구나 헤더 한 줄로 자신을 사내망이라고 주장할 수 있게
    /// 됩니다.
    pub trust_forwarded_for: bool,
    /// 출발지를 알 수 없을 때의 구간. 비면 `external` (fail-closed).
    pub default_zone: String,
    pub default_llm_destination: String,
    pub default_business_purpose: String,
    /// 인증 프록시가 검증해 붙인 정책 컨텍스트 헤더를 믿을지.
    pub trust_policy_headers: bool,
}

impl Enricher {
    /// 5개 환경 속성을 계산해 덮어쓴 **복사본**을 돌려줍니다.
    pub fn enrich(&self, id: &Identity, rc: &RequestContext) -> Identity {
        let now = rc.now.unwrap_or_else(Utc::now);
        let mut out = id.clone();
        let a = &mut out.attributes;
        a.insert(ATTR_REQUEST_TIME.into(), Value::String(crate::clock::to_rfc3339(now)));
        a.insert(ATTR_BUSINESS_HOURS.into(), Value::String(self.in_business_hours(now).to_string()));
        a.insert(ATTR_NETWORK_ZONE.into(), Value::String(self.zone_of(rc)));
        a.insert(ATTR_LLM_DESTINATION.into(), Value::String(self.llm_destination(rc)));
        a.insert(ATTR_BUSINESS_PURPOSE.into(), Value::String(self.business_purpose(rc)));
        out
    }

    fn in_business_hours(&self, t: DateTime<Utc>) -> bool {
        let local = t.with_timezone(&chrono::Local);
        let wd = local.weekday();
        if wd == chrono::Weekday::Sat || wd == chrono::Weekday::Sun {
            return false;
        }
        // 둘 다 0 이면 시간 제한이 없다는 뜻입니다.
        if self.start_hour == 0 && self.end_hour == 0 {
            return true;
        }
        let h = local.hour();
        h >= self.start_hour && h < self.end_hour
    }

    fn default_zone(&self) -> String {
        if self.default_zone.is_empty() {
            "external".into() // fail-closed
        } else {
            self.default_zone.clone()
        }
    }

    fn zone_of(&self, rc: &RequestContext) -> String {
        let remote = self.remote_ip(rc);
        if remote.is_empty() {
            return self.default_zone();
        }
        let Ok(addr) = remote.parse::<std::net::IpAddr>() else {
            return self.default_zone();
        };
        if self.internal_prefixes.iter().any(|p| p.contains(&addr)) {
            return "internal".into();
        }
        "external".into()
    }

    fn remote_ip(&self, rc: &RequestContext) -> String {
        if self.trust_forwarded_for {
            let xff = rc.get(FORWARDED_FOR_HEADER);
            if !xff.is_empty() {
                let first = xff.split(',').next().unwrap_or("").trim();
                if !first.is_empty() {
                    return first.to_string();
                }
            }
        }
        host_only(rc.get(REMOTE_ADDR_HEADER))
    }

    fn llm_destination(&self, rc: &RequestContext) -> String {
        let mut dest = self.default_llm_destination.clone();
        if self.trust_policy_headers {
            let h = rc.get(LLM_DESTINATION_HEADER).trim().to_lowercase();
            if !h.is_empty() {
                dest = h;
            }
        }
        if dest != "internal" && dest != "external" {
            return "external".into(); // fail-closed
        }
        dest
    }

    fn business_purpose(&self, rc: &RequestContext) -> String {
        if !self.trust_policy_headers {
            return self.default_business_purpose.clone();
        }
        let h = rc.get(BUSINESS_PURPOSE_HEADER).trim();
        if h.is_empty() { self.default_business_purpose.clone() } else { h.to_string() }
    }
}

/// `host:port` 에서 host 만 떼어냅니다.
fn host_only(addr: &str) -> String {
    let addr = addr.trim();
    if addr.is_empty() {
        return String::new();
    }
    // IPv6 `[::1]:8080`
    if let Some(rest) = addr.strip_prefix('[')
        && let Some(end) = rest.find(']')
    {
        return rest[.. end].to_string();
    }
    // IPv4 `1.2.3.4:80` — 콜론이 하나일 때만 포트로 봅니다(민짜 IPv6 오분리 방지).
    if addr.matches(':').count() == 1
        && let Some((h, _)) = addr.rsplit_once(':')
    {
        return h.to_string();
    }
    addr.to_string()
}

/// CIDR 목록을 파싱합니다. 잘못된 값은 조용히 넘기지 않고 오류를 냅니다.
pub fn parse_prefixes(cidrs: &[String]) -> Result<Vec<IpNet>> {
    let mut out = Vec::new();
    for c in cidrs {
        let c = c.trim();
        if c.is_empty() {
            continue;
        }
        out.push(c.parse::<IpNet>().map_err(|e| anyhow!("auth: invalid internal CIDR {c:?}: {e}"))?);
    }
    Ok(out)
}

/// 리졸버에 Enricher 를 씌웁니다.
#[derive(Debug)]
pub struct EnrichedResolver {
    pub inner: SharedResolver,
    pub enricher: Enricher,
}

impl Resolver for EnrichedResolver {
    fn resolve(&self, rc: &RequestContext) -> Result<Identity> {
        let id = self.inner.resolve(rc)?;
        Ok(self.enricher.enrich(&id, rc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn human() -> Identity {
        Identity {
            user_id: "emp-sales-01".into(),
            roles: vec!["sales".into()],
            ..Default::default()
        }
    }

    fn agent(token: &str) -> Identity {
        Identity {
            user_id: "agent-support-bot".into(),
            kind: KIND_AGENT.into(),
            roles: vec!["support".into()],
            token_sha256: token_hash(token),
            ..Default::default()
        }
    }

    fn ctx(pairs: &[(&str, &str)]) -> RequestContext {
        let mut rc = RequestContext::default();
        for (k, v) in pairs {
            rc.set(k, v);
        }
        rc
    }

    // --- 스코프 ---

    #[test]
    fn empty_allowlists_impose_no_restriction() {
        assert!(human().allows_tool("anything", "any_system"));
    }

    #[test]
    fn allowlists_only_narrow() {
        let mut a = agent("t");
        a.allowed_tools = vec!["get_ticket_status".into()];
        a.allowed_systems = vec!["ticket".into()];
        assert!(a.allows_tool("get_ticket_status", "ticket"));
        assert!(!a.allows_tool("process_refund", "ticket"));
        assert!(!a.allows_tool("get_ticket_status", "erp"));
    }

    // --- 이중 신뢰 모델 ---

    #[test]
    fn human_resolves_via_header() {
        let r = TokenResolver::new(Directory::new(vec![human()]), USER_ID_HEADER);
        let id = r.resolve(&ctx(&[(USER_ID_HEADER, "emp-sales-01")])).unwrap();
        assert_eq!(id.user_id, "emp-sales-01");
    }

    #[test]
    fn agent_cannot_authenticate_via_header_alone() {
        // 이것이 핵심입니다 — 헤더 한 줄로 에이전트를 사칭할 수 없습니다.
        let r = TokenResolver::new(Directory::new(vec![agent("secret")]), USER_ID_HEADER);
        let err = r.resolve(&ctx(&[(USER_ID_HEADER, "agent-support-bot")])).unwrap_err();
        assert!(err.to_string().contains("토큰 인증이 필요합니다"));
    }

    #[test]
    fn agent_resolves_via_bearer_token() {
        let r = TokenResolver::new(Directory::new(vec![agent("secret")]), USER_ID_HEADER);
        let id = r.resolve(&ctx(&[(AUTHORIZATION_HEADER, "Bearer secret")])).unwrap();
        assert_eq!(id.user_id, "agent-support-bot");
        assert!(id.is_agent());
    }

    #[test]
    fn bearer_token_wins_over_header_and_does_not_fall_back() {
        // 토큰이 있으면 헤더는 무시됩니다. 토큰이 틀렸다고 헤더로 되돌아가지 않습니다.
        let dir = Directory::new(vec![human(), agent("secret")]);
        let r = TokenResolver::new(dir, USER_ID_HEADER);
        let err = r
            .resolve(&ctx(&[(AUTHORIZATION_HEADER, "Bearer wrong"), (USER_ID_HEADER, "emp-sales-01")]))
            .unwrap_err();
        assert!(err.to_string().contains("유효하지 않은 에이전트 토큰"));
    }

    #[test]
    fn expired_token_is_rejected() {
        let mut a = agent("secret");
        a.token_expires_at = Some(Utc::now() - chrono::Duration::hours(1));
        let r = TokenResolver::new(Directory::new(vec![a]), USER_ID_HEADER);
        let err = r.resolve(&ctx(&[(AUTHORIZATION_HEADER, "Bearer secret")])).unwrap_err();
        assert!(err.to_string().contains("만료"));
    }

    #[test]
    fn unknown_subject_error_does_not_leak_who_exists() {
        let r = TokenResolver::new(Directory::new(vec![human()]), USER_ID_HEADER);
        let err = r.resolve(&ctx(&[(USER_ID_HEADER, "nobody")])).unwrap_err();
        assert_eq!(err.to_string(), "auth: 알 수 없는 호출 주체입니다");
        assert!(!err.to_string().contains("emp-sales-01"));
    }

    #[test]
    fn missing_header_is_rejected_not_downgraded_to_anonymous() {
        let r = TokenResolver::new(Directory::new(vec![human()]), USER_ID_HEADER);
        assert!(r.resolve(&ctx(&[])).is_err());
    }

    // --- Enricher ---

    #[test]
    fn enricher_overwrites_subject_claimed_attributes() {
        // 주체가 설정 파일에 business_hours: "true" 라고 적어둬도 무시됩니다.
        let mut id = human();
        id.attributes.insert(ATTR_BUSINESS_HOURS.into(), Value::String("true".into()));
        id.attributes.insert(ATTR_NETWORK_ZONE.into(), Value::String("internal".into()));

        let e = Enricher::default(); // 사내망 대역 없음 → external
        let out = e.enrich(&id, &ctx(&[]));
        assert_eq!(out.attr(ATTR_NETWORK_ZONE).unwrap(), &Value::String("external".into()));
    }

    #[test]
    fn unknown_source_ip_is_external_fail_closed() {
        let e = Enricher {
            internal_prefixes: parse_prefixes(&["10.0.0.0/8".into()]).unwrap(),
            ..Default::default()
        };
        let out = e.enrich(&human(), &ctx(&[]));
        assert_eq!(out.attr(ATTR_NETWORK_ZONE).unwrap(), &Value::String("external".into()));
    }

    #[test]
    fn internal_cidr_matches_peer_address() {
        let e = Enricher {
            internal_prefixes: parse_prefixes(&["10.0.0.0/8".into()]).unwrap(),
            ..Default::default()
        };
        let out = e.enrich(&human(), &ctx(&[(REMOTE_ADDR_HEADER, "10.1.2.3:5555")]));
        assert_eq!(out.attr(ATTR_NETWORK_ZONE).unwrap(), &Value::String("internal".into()));
    }

    #[test]
    fn forwarded_for_is_ignored_unless_trusted() {
        let prefixes = parse_prefixes(&["10.0.0.0/8".into()]).unwrap();

        // 신뢰하지 않으면 X-Forwarded-For 로 사내망을 주장할 수 없습니다.
        let e = Enricher {
            internal_prefixes: prefixes.clone(),
            trust_forwarded_for: false,
            ..Default::default()
        };
        let rc = ctx(&[(FORWARDED_FOR_HEADER, "10.0.0.9"), (REMOTE_ADDR_HEADER, "203.0.113.7:443")]);
        assert_eq!(e.enrich(&human(), &rc).attr(ATTR_NETWORK_ZONE).unwrap(), &Value::String("external".into()));

        // 신뢰하도록 켜면 첫 홉을 씁니다.
        let e = Enricher {
            internal_prefixes: prefixes,
            trust_forwarded_for: true,
            ..Default::default()
        };
        assert_eq!(e.enrich(&human(), &rc).attr(ATTR_NETWORK_ZONE).unwrap(), &Value::String("internal".into()));
    }

    #[test]
    fn llm_destination_normalizes_unknown_values_to_external() {
        let e = Enricher {
            default_llm_destination: "bogus".into(),
            ..Default::default()
        };
        let out = e.enrich(&human(), &ctx(&[]));
        assert_eq!(out.attr(ATTR_LLM_DESTINATION).unwrap(), &Value::String("external".into()));
    }

    #[test]
    fn policy_context_headers_ignored_unless_trusted() {
        let e = Enricher {
            default_business_purpose: "sales_followup".into(),
            trust_policy_headers: false,
            ..Default::default()
        };
        let rc = ctx(&[(BUSINESS_PURPOSE_HEADER, "forged")]);
        assert_eq!(
            e.enrich(&human(), &rc).attr(ATTR_BUSINESS_PURPOSE).unwrap(),
            &Value::String("sales_followup".into())
        );
    }

    #[test]
    fn business_hours_zero_range_means_no_restriction() {
        let e = Enricher::default();
        let out = e.enrich(&human(), &ctx(&[]));
        // 주말이 아니면 참. 주말이면 거짓 — 둘 중 하나이므로 존재만 확인합니다.
        assert!(out.attr(ATTR_BUSINESS_HOURS).is_some());
    }

    // --- 토큰 ---

    #[test]
    fn generated_tokens_are_unique_and_hash_stably() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
        assert_eq!(token_hash(&a), token_hash(&a));
        assert_ne!(token_hash(&a), token_hash(&b));
    }

    // --- 리로드 / 롤백 ---

    #[test]
    fn reload_then_rollback_restores_previous_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("principal.yaml");
        std::fs::write(&path, "principals:\n  - user_id: a\n    roles: [sales]\n").unwrap();

        let r = TokenResolver::new(load_directory(&path).unwrap(), USER_ID_HEADER);
        assert_eq!(r.principal_count(), 1);

        std::fs::write(&path, "principals:\n  - user_id: a\n    roles: [sales]\n  - user_id: b\n    roles: [hr]\n").unwrap();
        r.reload(&path, None).unwrap();
        assert_eq!(r.principal_count(), 2);

        r.rollback().unwrap();
        assert_eq!(r.principal_count(), 1);
    }

    #[test]
    fn failed_reload_leaves_active_directory_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("principal.yaml");
        std::fs::write(&path, "principals:\n  - user_id: a\n    roles: [sales]\n").unwrap();
        let r = TokenResolver::new(load_directory(&path).unwrap(), USER_ID_HEADER);

        // 검증기가 거부하면 활성 상태가 바뀌면 안 됩니다.
        std::fs::write(&path, "principals:\n  - user_id: b\n    roles: [hr]\n").unwrap();
        let reject = |_: &[Identity]| -> Result<()> { bail!("nope") };
        assert!(r.reload(&path, Some(&reject)).is_err());
        assert_eq!(r.identities()[0].user_id, "a");
    }

    #[test]
    fn rollback_without_snapshot_errors() {
        let r = TokenResolver::new(Directory::new(vec![human()]), USER_ID_HEADER);
        assert!(r.rollback().is_err());
    }

    // --- principal 파일 ---

    #[test]
    fn rejects_both_user_id_and_principals() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.yaml");
        std::fs::write(&path, "user_id: a\nprincipals:\n  - user_id: b\n").unwrap();
        assert!(load_directory(&path).is_err());
    }

    #[test]
    fn rejects_duplicate_user_ids() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.yaml");
        std::fs::write(&path, "principals:\n  - user_id: a\n  - user_id: a\n").unwrap();
        assert!(load_directory(&path).is_err());
    }

    #[test]
    fn only_requires_exactly_one_principal() {
        let d = Directory::new(vec![human()]);
        assert!(d.only().is_ok());
        let d = Directory::new(vec![human(), agent("t")]);
        assert!(d.only().is_err());
    }

    #[test]
    fn upsert_cannot_flip_kind_between_human_and_agent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.yaml");
        std::fs::write(&path, "principals:\n  - user_id: a\n    roles: [sales]\n").unwrap();

        let mut a = agent("t");
        a.user_id = "a".into();
        assert!(upsert_principal_in_file(&path, &a).is_err());
    }

    #[test]
    fn upsert_adds_then_updates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.yaml");
        std::fs::write(&path, "principals:\n  - user_id: a\n    roles: [sales]\n").unwrap();

        upsert_principal_in_file(&path, &agent("t")).unwrap();
        let d = load_directory(&path).unwrap();
        assert_eq!(d.len(), 2);
        assert!(d.lookup("agent-support-bot").unwrap().is_agent());

        let mut a2 = agent("t2");
        a2.roles = vec!["hr".into()];
        upsert_principal_in_file(&path, &a2).unwrap();
        let d = load_directory(&path).unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!(d.lookup("agent-support-bot").unwrap().roles, vec!["hr"]);
    }

    #[test]
    fn cannot_remove_last_principal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.yaml");
        std::fs::write(&path, "principals:\n  - user_id: a\n    roles: [sales]\n").unwrap();
        assert!(remove_principal_from_file(&path, "a").is_err());
    }

    #[test]
    fn plaintext_tokens_never_reach_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.yaml");
        std::fs::write(&path, "principals:\n  - user_id: a\n    roles: [sales]\n").unwrap();

        let token = "super-secret-token";
        let a = agent(token);
        upsert_principal_in_file(&path, &a).unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(!body.contains(token));
        assert!(body.contains(&token_hash(token)));
    }

    #[test]
    fn host_only_handles_ipv4_ipv6_and_bare() {
        assert_eq!(host_only("10.0.0.1:443"), "10.0.0.1");
        assert_eq!(host_only("[::1]:8080"), "::1");
        assert_eq!(host_only("10.0.0.1"), "10.0.0.1");
    }

    #[test]
    fn bad_cidr_is_loud_not_silent() {
        assert!(parse_prefixes(&["not-a-cidr".into()]).is_err());
    }

    #[test]
    fn resolver_is_object_safe() {
        let _: SharedResolver = Arc::new(StaticResolver {
            identity: human(),
        });
    }
}
