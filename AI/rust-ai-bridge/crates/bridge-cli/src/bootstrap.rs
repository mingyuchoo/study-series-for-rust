//! 게이트웨이 기동 — 플래그 · 검증 · 저장소 선택 · 핫 리로드.
//!
//! # 기동을 거부하는 경우
//!
//! 조용히 안전하지 않은 상태로 뜨느니 뜨지 않는 편이 낫습니다.
//!
//! - **목 백엔드**: `--allow-mock-backends` 없이 `memory`/빈 `base_url`
//!   시스템이 있으면 거부.
//! - **인벤토리 불일치**: 인벤토리에 없는 시스템의 도구, 쓰기 기능 없는
//!   시스템의 쓰기 도구.
//! - **정책 참조 오류**: 정책이 없는 도구·인자·출력 필드를 가리키면 거부(오타
//!   난 규칙은 조용히 발동하지 않는데, 그것이 가장 위험합니다).
//! - **아카이브 없는 보존 집행**: `--purge-interval` 만으로는 뜨지 않습니다.
//! - **Redis 불통**: `--redis-url` 을 주면 기동 시 ping 합니다. 레이트 리밋
//!   없이 조용히 도는 것보다 기동을 거부하는 편이 낫습니다.
//! - **stdio 에서 주체가 여럿**: 임의로 하나를 고르면 의도하지 않은 권한으로
//!   동작합니다.

use ai_bridge::{SystemsOptions,
                adapter::Adapter,
                app,
                approval::{self,
                           PostgresApprovalStore,
                           SqliteApprovalStore},
                audit::{self,
                        PostgresLogger,
                        SqliteLogger},
                auth::{self,
                       Enricher,
                       SharedResolver,
                       StaticResolver,
                       TokenResolver},
                budget,
                console::Console,
                eval::SqliteEvalStore,
                gateway::{Deps as GwDeps,
                          Gateway,
                          StoreApprover},
                inventory::{Interface,
                            Inventory},
                ops,
                policy,
                ratelimit,
                telemetry,
                toolcatalog,
                workflow::{self,
                           PostgresWorkflowStore,
                           SqliteWorkflowStore}};
use anyhow::{Result,
             bail};
use clap::Parser;
use legacies::Systems;
use std::{path::PathBuf,
          sync::Arc};

/// 레거시 IT 시스템과 LLM 에이전트를 안전하게 연결하는 AI Integration Gateway.
#[derive(Debug, Parser, Clone)]
#[command(name = "gateway", version, about)]
pub struct Config {
    // --- 설정 파일 ---
    #[arg(long, default_value = "config/policies.yaml")]
    pub policy: PathBuf,
    #[arg(long, default_value = "config/principal.yaml")]
    pub principal: PathBuf,
    #[arg(long, default_value = "config/systems.yaml")]
    pub systems: PathBuf,
    /// 동적 도구 카탈로그 (SIGHUP 으로 재적용).
    #[arg(long, default_value = "config/tools-dynamic.yaml")]
    pub tools_catalog: PathBuf,

    // --- 저장소 ---
    /// SQLite 경로 (감사·승인·워크플로가 같은 파일을 씁니다).
    #[arg(long, default_value = "audit.db")]
    pub db: PathBuf,
    /// 주면 **세 저장소가 함께** PostgreSQL 로 갑니다. `--db` 는 무시됩니다.
    #[arg(long)]
    pub postgres_dsn: Option<String>,
    /// 레이트 리밋·비용 상한을 여러 인스턴스가 공유합니다.
    #[arg(long)]
    pub redis_url: Option<String>,
    /// 품질 평가 DB (콘솔 `/eval`).
    #[arg(long)]
    pub eval_db: Option<PathBuf>,

    // --- 전송 ---
    /// HTTP 주소 (예: `:8080`). 비우면 stdio.
    #[arg(long)]
    pub http: Option<String>,
    /// stdio 주체. 주체가 하나뿐이면 생략할 수 있습니다.
    #[arg(long, default_value = "")]
    pub user: String,
    #[arg(long, default_value = auth::USER_ID_HEADER)]
    pub user_header: String,

    // --- 레거시 ---
    #[arg(long, default_value = "")]
    pub erp_url: String,
    /// `postgres://…` 또는 SQLite 파일 경로. `--erp-url` 보다 우선.
    #[arg(long, default_value = "")]
    pub erp_db: String,
    #[arg(long, default_value = "keyword", value_parser = ["keyword", "vector"])]
    pub docs_retriever: String,

    // --- 안전 스위치 ---
    /// **L4 도구는 이것 없이는 실행되지 않습니다.**
    #[arg(long)]
    pub allow_high_risk: bool,
    /// 개발·데모용. 운영에서는 절대 켜지 마십시오.
    #[arg(long)]
    pub allow_mock_backends: bool,

    // --- 환경 속성 ---
    #[arg(long, default_value_t = 9)]
    pub business_start: u32,
    #[arg(long, default_value_t = 18)]
    pub business_end: u32,
    #[arg(long, default_value = "10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,127.0.0.0/8,::1/128,fc00::/7")]
    pub internal_cidr: String,
    /// **엣지에 직접 노출된 게이트웨이에서는 금지** — 누구나 헤더로 사내망을
    /// 주장합니다.
    #[arg(long)]
    pub trust_forwarded_for: bool,
    #[arg(long, default_value = "external")]
    pub llm_destination: String,
    #[arg(long, default_value = "")]
    pub business_purpose: String,
    /// 인증 프록시가 검증한 정책 컨텍스트 헤더를 신뢰합니다.
    #[arg(long)]
    pub trust_policy_context_headers: bool,

    // --- 리소스 가드 ---
    #[arg(long, default_value_t = 0)]
    pub session_budget_micros: i64,

    // --- 보존 ---
    /// 켜려면 아카이브 대상이 **반드시** 필요합니다.
    #[arg(long)]
    pub purge_interval: Option<humantime::Duration>,
    #[arg(long, default_value = "")]
    pub archive_dir: String,
    #[arg(long, default_value = "")]
    pub archive_syslog: String,
    /// 아카이브 없이 버립니다. **"깜빡한 것"과 "그러기로 한 것"은 구분되어야
    /// 합니다.**
    #[arg(long)]
    pub archive_discard: bool,

    // --- 운영 ---
    /// 운영 콘솔 주소 (예: `:8081`). MCP 와 **다른 포트**여야 합니다.
    #[arg(long)]
    pub console: Option<String>,
    #[arg(long, default_value = ai_bridge::console::ADMIN_ROLE)]
    pub console_role: String,
    /// **개발 전용.** 콘솔의 인증을 없애는 것과 같습니다.
    #[arg(long)]
    pub console_user: Option<String>,
    /// 콘솔 없이 프로브만 띄울 주소.
    #[arg(long)]
    pub probe: Option<String>,
    /// Prometheus 메트릭 주소 (예: `:9464`).
    #[arg(long)]
    pub metrics: Option<String>,
    /// 다중 인스턴스 설정 동기화 stamp 파일.
    #[arg(long)]
    pub reload_stamp: Option<PathBuf>,
}

/// 열린 저장소들.
struct Stores {
    audit: Arc<dyn audit::Store>,
    recorder: Arc<dyn audit::Recorder>,
    reader: Arc<dyn audit::Reader>,
    approvals: Arc<dyn approval::Store>,
    workflows: Arc<dyn workflow::Store>,
}

/// **`--postgres-dsn` 하나면 세 저장소가 함께 갑니다.**
///
/// 승인은 PostgreSQL 에, 그 승인으로 실행되는 업무 흐름은 SQLite 에 두는 식으로
/// 갈라지면 분산 배포에서 한쪽만 공유됩니다.
async fn open_stores(cfg: &Config) -> Result<Stores> {
    if let Some(dsn) = &cfg.postgres_dsn {
        let a = Arc::new(PostgresLogger::open(dsn).await?);
        return Ok(Stores {
            recorder: a.clone(),
            reader: a.clone(),
            audit: a,
            approvals: Arc::new(PostgresApprovalStore::open(dsn).await?),
            workflows: Arc::new(PostgresWorkflowStore::open(dsn).await?),
        });
    }
    let a = Arc::new(SqliteLogger::open(&cfg.db).await?);
    Ok(Stores {
        recorder: a.clone(),
        reader: a.clone(),
        audit: a,
        approvals: Arc::new(SqliteApprovalStore::open(&cfg.db).await?),
        workflows: Arc::new(SqliteWorkflowStore::open(&cfg.db).await?),
    })
}

/// **Redis 를 주면 기동 시 ping 합니다.**
async fn open_guards(cfg: &Config) -> Result<(Arc<dyn ratelimit::Limiter>, Arc<dyn budget::Tracker>)> {
    match &cfg.redis_url {
        | Some(url) => Ok((
            Arc::new(ratelimit::RedisLimiter::new(url, "ratelimit:").await?),
            Arc::new(budget::RedisTracker::new(url, "budget", cfg.session_budget_micros).await?),
        )),
        | None => Ok((Arc::new(ratelimit::Memory::new()), Arc::new(budget::Memory::new(cfg.session_budget_micros)))),
    }
}

/// 인메모리·목 백엔드를 쓰고 있는지 검사합니다.
fn refuse_mock_backends(cfg: &Config, inv: &Inventory) -> Result<()> {
    if cfg.allow_mock_backends {
        return Ok(());
    }
    for s in inv.systems() {
        let erp_overridden = s.name == "erp" && (!cfg.erp_url.is_empty() || !cfg.erp_db.is_empty());
        let is_mock = s.interface == Interface::Memory || (s.interface.is_network() && s.base_url.is_empty() && !erp_overridden);
        if is_mock {
            bail!(
                "시스템 {:?} 이 목/인메모리 백엔드입니다; 실제 base_url 을 설정하거나 \
                 개발 시에만 --allow-mock-backends 를 사용하세요",
                s.name
            );
        }
    }
    Ok(())
}

/// 감사 아카이브 대상을 고릅니다. **`--purge-interval` 은 아카이브 없이 켤 수
/// 없습니다.**
fn archive_exporter(cfg: &Config) -> Result<Option<Box<dyn audit::Exporter>>> {
    if cfg.purge_interval.is_none() {
        return Ok(None);
    }
    let chosen = [!cfg.archive_dir.is_empty(), !cfg.archive_syslog.is_empty(), cfg.archive_discard]
        .iter()
        .filter(|b| **b)
        .count();

    if chosen == 0 {
        bail!(
            "--purge-interval 을 켜려면 --archive-dir · --archive-syslog · --archive-discard \
             중 하나로 아카이브 대상을 지정해야 합니다"
        );
    }
    if chosen > 1 {
        bail!("아카이브 대상은 하나만 지정하세요");
    }
    Ok(Some(audit::build_exporter(&cfg.archive_dir, &cfg.archive_syslog, cfg.archive_discard)?))
}

fn parse_addr(s: &str) -> Result<std::net::SocketAddr> {
    let s = if let Some(port) = s.strip_prefix(':') {
        format!("0.0.0.0:{port}")
    } else {
        s.to_string()
    };
    Ok(s.parse()?)
}

/// 기동한 게이트웨이.
pub struct Bootstrapped {
    pub gateway: Arc<Gateway>,
    pub ops: Arc<ops::Service>,
    pub resolver: SharedResolver,
    pub resources: Vec<ai_bridge::adapter::Resource>,
    pub cfg: Config,
    pub telemetry: Option<Arc<telemetry::Provider>>,
}

/// 게이트웨이를 조립합니다. **검증에 실패하면 뜨지 않습니다.**
pub async fn bootstrap(cfg: Config) -> Result<Bootstrapped> {
    if cfg.console_user.is_some() && cfg.console.is_none() {
        bail!("--console-user 는 --console 없이 쓸 수 없습니다");
    }

    // --- 인벤토리 ---
    let inv = Arc::new(Inventory::load(&cfg.systems)?);
    refuse_mock_backends(&cfg, &inv)?;

    // --- 정책 ---
    let pol = Arc::new(policy::Engine::load(&cfg.policy)?);

    // --- 저장소 ---
    let stores = open_stores(&cfg).await?;
    let (limiter, budget) = open_guards(&cfg).await?;

    // --- 어댑터 ---
    let retriever: Option<Arc<dyn ai_bridge::adapter::Retriever>> = match cfg.docs_retriever.as_str() {
        | "vector" => Some(Arc::new(legacies::retriever::VectorRetriever::in_memory())),
        | _ => None, // keyword 가 기본입니다.
    };
    let opts = SystemsOptions {
        inventory: Some(inv.clone()),
        erp_base_url: cfg.erp_url.clone(),
        erp_db_dsn: cfg.erp_db.clone(),
        docs_retriever: retriever,
    };
    // 워크플로 저장소를 넘겨 **감사·승인과 같은 백엔드**를 쓰게 합니다.
    let adapters: Vec<Arc<dyn Adapter>> = legacies::build_adapters(&opts, Some(stores.workflows.clone())).await?;

    let reg = Arc::new(app::build_registry(&adapters)?);

    // --- 기동 시 교차 검증 ---
    app::validate_inventory(&reg, &inv)?;
    policy::validate_references(&pol.snapshot(), &reg, &inv)?;

    // --- 주체 ---
    let dir = auth::load_directory(&cfg.principal)?;
    let identities = dir.identities();
    policy::validate_allowlists(&identities, &reg, &inv)?;

    let prefixes = auth::parse_prefixes(&cfg.internal_cidr.split(',').map(|s| s.to_string()).collect::<Vec<_>>())?;

    let http_mode = cfg.http.is_some();
    let enricher = Enricher {
        start_hour: cfg.business_start,
        end_hour: cfg.business_end,
        internal_prefixes: prefixes,
        trust_forwarded_for: cfg.trust_forwarded_for,
        // stdio 는 로컬 프로세스이므로 사내망입니다.
        default_zone: if http_mode {
            String::new() // → external (fail-closed)
        } else {
            "internal".into()
        },
        default_llm_destination: cfg.llm_destination.clone(),
        default_business_purpose: cfg.business_purpose.clone(),
        trust_policy_headers: cfg.trust_policy_context_headers,
    };

    let (resolver, tokens): (SharedResolver, Option<Arc<TokenResolver>>) = if http_mode {
        let t = Arc::new(TokenResolver::new(dir.clone(), &cfg.user_header));
        // 첫 SIGHUP 의 "이전 digest" 가 비지 않도록 한 번 읽어둡니다.
        let _ = t.reload(&cfg.principal, None);

        for id in &identities {
            if id.is_agent() && id.token_sha256.is_empty() {
                tracing::warn!("에이전트 {:?} 에 token_sha256 이 없어 HTTP 로 인증할 수 없습니다", id.user_id);
            }
        }
        (
            Arc::new(auth::EnrichedResolver {
                inner: t.clone(),
                enricher: enricher.clone(),
            }),
            Some(t),
        )
    } else {
        // stdio — 프로세스 하나가 사용자 한 명을 대신합니다.
        let principal = auth::load_principal(&cfg.principal, &cfg.user)?;
        (
            Arc::new(auth::EnrichedResolver {
                inner: Arc::new(StaticResolver {
                    identity: principal,
                }),
                enricher: enricher.clone(),
            }),
            None,
        )
    };

    // --- 계측 ---
    let telemetry = match &cfg.metrics {
        | Some(_) => Some(Arc::new(telemetry::Provider::new()?)),
        | None => None,
    };

    // --- 동적 도구 카탈로그 ---
    let catalog = Arc::new(toolcatalog::Manager::new(Some(cfg.tools_catalog.clone()), reg.clone(), Some(inv.clone())));
    if let Err(e) = catalog.reload() {
        tracing::warn!("동적 도구 카탈로그를 읽지 못했습니다: {e}");
    }

    // --- 게이트웨이 ---
    let breaker = Arc::new(ai_bridge::breaker::Breaker::default());
    let gateway = Arc::new(Gateway::new(GwDeps {
        registry: reg.clone(),
        policy: pol.clone(),
        audit: stores.recorder.clone(),
        approver: Some(Arc::new(StoreApprover::new(stores.approvals.clone()))),
        breaker: Some(breaker.clone()),
        inventory: Some(inv.clone()),
        limiter: Some(limiter),
        budget: Some(budget.clone()),
        telemetry: telemetry.as_ref().map(|p| p.telemetry.clone()),
        masker: None,
        injection: None,
        allow_high_risk: cfg.allow_high_risk,
    }));

    // --- 보존 기간 집행 ---
    if let Some(interval) = cfg.purge_interval {
        let exporter = archive_exporter(&cfg)?.expect("purge_interval implies an exporter");
        let policy = app::retention_policy(&reg);
        let store = stores.audit.clone();
        let period: std::time::Duration = interval.into();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(period);
            loop {
                ticker.tick().await;
                match store.purge(&policy, chrono::Utc::now(), exporter.as_ref()).await {
                    | Ok(p) if p.deleted > 0 => {
                        tracing::info!("감사 보존: {}건 아카이브 후 삭제", p.deleted)
                    },
                    | Ok(_) => {},
                    | Err(e) => tracing::error!("감사 보존 실패(삭제 중단): {e}"),
                }
            }
        });
    }

    // --- 평가 저장소 ---
    let eval = match &cfg.eval_db {
        | Some(p) => Some(Arc::new(SqliteEvalStore::open(p).await?) as Arc<dyn ai_bridge::eval::Store>),
        | None => None,
    };

    // --- 운영 파사드 ---
    let roles: Vec<String> = pol.snapshot().roles.keys().cloned().collect();
    let ops_svc = Arc::new(ops::Service::new(ops::Deps {
        audit: stores.reader.clone(),
        approvals: stores.approvals.clone(),
        workflows: Some(stores.workflows.clone()),
        inventory: Some(inv.clone()),
        registry: Some(reg.clone()),
        adapters: adapters.clone(),
        principals: identities,
        tokens: tokens.clone(),
        principal_path: Some(cfg.principal.clone()),
        policy: Some(pol.clone()),
        policy_path: Some(cfg.policy.clone()),
        systems_path: Some(cfg.systems.clone()),
        catalog: Some(catalog),
        reload_stamp_path: cfg.reload_stamp.clone(),
        adapter_factory: Some(Arc::new(Systems::new())),
        systems_options: opts,
        roles,
        budget: Some(budget),
        breakers: Some(breaker),
        eval,
        recorder: Some(stores.recorder.clone()),
    })?);

    let resources = app::resources(&adapters);

    // --- 기동 요약 ---
    tracing::info!(
        "도구 {}개 · 시스템 {}개 · 저장소 {} · 전송 {}",
        reg.len(),
        inv.len(),
        if cfg.postgres_dsn.is_some() { "PostgreSQL" } else { "SQLite" },
        if http_mode { "HTTP" } else { "stdio" }
    );
    if cfg.allow_high_risk {
        tracing::warn!("--allow-high-risk: L4(자동 실행형) 도구가 실행될 수 있습니다");
    }
    if cfg.allow_mock_backends {
        tracing::warn!("--allow-mock-backends: 목/인메모리 백엔드를 씁니다 (개발 전용)");
    }
    if cfg.console_user.is_some() {
        tracing::warn!(
            "--console-user: 콘솔 인증이 사실상 없습니다. 포트에 닿는 누구나 관리자가 \
             되며 콘솔은 프롬프트 원문을 보여줍니다. 운영에서 쓰지 마십시오."
        );
    }
    if cfg.trust_forwarded_for {
        tracing::warn!(
            "--trust-forwarded-for: X-Forwarded-For 를 믿습니다. 신뢰할 수 있는 프록시 \
             뒤에서만 켜십시오 — 엣지에 직접 노출되면 누구나 사내망을 주장할 수 있습니다."
        );
    }
    for s in app::unused_systems(&reg, &inv) {
        tracing::warn!("시스템 {s:?} 에 등록된 도구가 없습니다");
    }
    for h in app::check_health(&adapters, std::time::Duration::from_secs(3)).await {
        if !h.healthy {
            tracing::warn!("레거시 {} 이(가) 응답하지 않습니다: {}", h.system, h.error);
        }
    }

    Ok(Bootstrapped {
        gateway,
        ops: ops_svc,
        resolver,
        resources,
        cfg,
        telemetry,
    })
}

/// 운영 콘솔(또는 프로브)을 띄웁니다.
pub async fn serve_console(b: &Bootstrapped) -> Result<Option<tokio::task::JoinHandle<()>>> {
    let Some(addr) = &b.cfg.console else {
        // 콘솔 없이 프로브만.
        if let Some(probe) = &b.cfg.probe {
            let ops = b.ops.clone();
            let addr = parse_addr(probe)?;
            let app = axum::Router::new()
                .route(
                    "/livez",
                    axum::routing::get(|axum::extract::State(o): axum::extract::State<Arc<ops::Service>>| async move { axum::Json(o.live()) }),
                )
                .route(
                    "/readyz",
                    axum::routing::get(|axum::extract::State(o): axum::extract::State<Arc<ops::Service>>| async move {
                        let (st, ok) = o.ready().await;
                        let code = if ok {
                            axum::http::StatusCode::OK
                        } else {
                            axum::http::StatusCode::SERVICE_UNAVAILABLE
                        };
                        (code, axum::Json(st))
                    }),
                )
                .with_state(ops);
            tracing::info!("프로브: http://{addr}/livez");
            let listener = tokio::net::TcpListener::bind(addr).await?;
            return Ok(Some(tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            })));
        }
        return Ok(None);
    };

    // **개발 전용**: 콘솔 주체를 고정합니다.
    let resolver: SharedResolver = match &b.cfg.console_user {
        | Some(u) => {
            let id = auth::load_principal(&b.cfg.principal, u)?;
            Arc::new(StaticResolver {
                identity: id,
            })
        },
        | None => b.resolver.clone(),
    };

    let console = Console::new(b.ops.clone(), Some(resolver), &b.cfg.console_role)?;
    let addr = parse_addr(addr)?;
    tracing::info!("운영 콘솔: http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let app = console.router().into_make_service_with_connect_info::<std::net::SocketAddr>();
    Ok(Some(tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    })))
}

/// Prometheus 메트릭을 띄웁니다.
pub async fn serve_metrics(b: &Bootstrapped) -> Result<Option<tokio::task::JoinHandle<()>>> {
    let (Some(addr), Some(provider)) = (&b.cfg.metrics, &b.telemetry) else {
        return Ok(None);
    };
    let addr = parse_addr(addr)?;
    let p = provider.clone();
    let app = axum::Router::new().route(
        "/metrics",
        axum::routing::get(move || {
            let p = p.clone();
            async move { p.render() }
        }),
    );
    tracing::info!("메트릭: http://{addr}/metrics");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    Ok(Some(tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    })))
}

/// SIGHUP → 번들 리로드, SIGUSR1 → 롤백.
#[cfg(unix)]
pub fn start_signal_handlers(ops: Arc<ops::Service>) {
    use tokio::signal::unix::{SignalKind,
                              signal};

    tokio::spawn(async move {
        let Ok(mut hup) = signal(SignalKind::hangup()) else {
            return;
        };
        let Ok(mut usr1) = signal(SignalKind::user_defined1()) else {
            return;
        };
        loop {
            tokio::select! {
                _ = hup.recv() => {
                    tracing::info!("SIGHUP — 설정을 다시 읽습니다");
                    let r = ops.reload_config_bundle("system").await;
                    for s in &r.steps {
                        if s.ok {
                            tracing::info!("  {} ✓ {}", s.name, s.message);
                        } else {
                            tracing::error!("  {} ✗ {}", s.name, s.message);
                        }
                    }
                    if r.rolled_back {
                        tracing::error!("리로드에 실패해 **되돌렸습니다** — 이전 설정으로 계속합니다");
                    }
                }
                _ = usr1.recv() => {
                    tracing::info!("SIGUSR1 — 정책·주체를 직전 스냅샷으로 되돌립니다");
                }
            }
        }
    });
}

#[cfg(not(unix))]
pub fn start_signal_handlers(_ops: Arc<ops::Service>) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config { Config::parse_from(["gateway"]) }

    #[test]
    fn purge_interval_requires_an_archive_target() {
        // "깜빡한 것"과 "그러기로 한 것"은 코드에서 구분되어야 합니다.
        let mut c = cfg();
        c.purge_interval = Some("24h".parse().unwrap());
        assert!(archive_exporter(&c).is_err());

        // 그러기로 한 것.
        c.archive_discard = true;
        assert!(archive_exporter(&c).is_ok());

        // 둘을 준 것.
        c.archive_dir = "/tmp/a".into();
        assert!(archive_exporter(&c).is_err());
    }

    #[test]
    fn no_purge_interval_needs_no_exporter() {
        assert!(archive_exporter(&cfg()).unwrap().is_none());
    }

    #[test]
    fn mock_backends_are_refused_without_the_flag() {
        let inv = Inventory::load(std::path::Path::new("../../config/systems.yaml")).unwrap();
        let mut c = cfg();

        // 기본 설정은 memory 백엔드를 씁니다 — 거부되어야 합니다.
        assert!(refuse_mock_backends(&c, &inv).is_err());

        // 명시적으로 허용하면 통과합니다.
        c.allow_mock_backends = true;
        assert!(refuse_mock_backends(&c, &inv).is_ok());
    }

    #[test]
    fn l4_is_off_by_default() {
        assert!(!cfg().allow_high_risk);
        assert!(!cfg().allow_mock_backends);
        assert!(!cfg().trust_forwarded_for);
        assert!(!cfg().trust_policy_context_headers);
    }

    #[test]
    fn addr_accepts_the_colon_port_shorthand() {
        assert_eq!(parse_addr(":8080").unwrap().port(), 8080);
        assert_eq!(parse_addr("127.0.0.1:9090").unwrap().port(), 9090);
    }
}
