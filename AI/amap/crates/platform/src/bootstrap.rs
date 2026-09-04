//! Wire the platform together from settings: knowledge store, evidence lake, event bus,
//! policy engine and LLM access (embedded gateway, remote gateway, or fixture-driven mock).
use crate::audit::KnowledgeAuditSink;
use crate::runspec::RunSpec;
use crate::settings::Settings;
use amap_domain::*;
use amap_evidence::EvidenceLake;
use amap_knowledge::{InMemoryKnowledgeStore, KnowledgeStore};
use amap_llm::{
    AnthropicProvider, AzureOpenAiProvider, Gateway, GatewayConfig, LlmClient, MockProvider,
    OpenAiProvider, Router, RouterConfig, TaskKind,
};
use amap_orchestrator::{
    AgentContext, Clock, EventBus, IdGenerator, InMemoryBus, RunConfig, SystemClock, UuidGenerator,
};
use amap_policy::PolicyEngine;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

pub struct Platform {
    pub settings: Settings,
    pub knowledge: Arc<dyn KnowledgeStore>,
    pub lake: Arc<EvidenceLake>,
    pub bus: Arc<dyn EventBus>,
    pub policy: Arc<PolicyEngine>,
    pub clock: Arc<dyn Clock>,
    pub ids: Arc<dyn IdGenerator>,
    pub llm: Arc<dyn LlmClient>,
    pub gateway: Option<Arc<Gateway>>,
    pub mock: bool,
}

pub struct PlatformBuilder {
    settings: Settings,
    mock_fixtures: Option<std::path::PathBuf>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
}

impl PlatformBuilder {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            mock_fixtures: None,
            clock: Arc::new(SystemClock),
            ids: Arc::new(UuidGenerator),
        }
    }
    /// Use fixture-driven mock LLM answers (offline demo / CI).
    pub fn with_mock_fixtures(mut self, dir: Option<&Path>) -> Self {
        self.mock_fixtures = Some(dir.map(|d| d.to_path_buf()).unwrap_or_default());
        self
    }

    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    pub fn with_ids(mut self, ids: Arc<dyn IdGenerator>) -> Self {
        self.ids = ids;
        self
    }

    pub async fn build(self) -> anyhow::Result<Platform> {
        let s = self.settings.clone();
        let knowledge = open_knowledge(&s).await?;
        let audit_sink: Arc<dyn amap_llm::AuditSink> =
            Arc::new(KnowledgeAuditSink::new(knowledge.clone()));
        let lake = Arc::new(if s.lake.starts_with("s3://") {
            EvidenceLake::open_s3(&s.lake).await?
        } else {
            EvidenceLake::open_local(&s.lake).await?
        });
        let bus: Arc<dyn EventBus> = match &s.nats_url {
            Some(url) => match amap_orchestrator::NatsBus::connect(url, "amap").await {
                Ok(b) => {
                    tracing::info!(%url, "event bus: NATS JetStream");
                    Arc::new(b)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "NATS unavailable; using in-memory bus");
                    Arc::new(InMemoryBus::new())
                }
            },
            None => Arc::new(InMemoryBus::new()),
        };
        let policy = Arc::new(PolicyEngine::default());
        let clock = self.clock.clone();
        let ids = self.ids.clone();

        let mut mock = false;
        let (llm, gateway): (Arc<dyn LlmClient>, Option<Arc<Gateway>>) = if let Some(fixtures) =
            &self.mock_fixtures
        {
            mock = true;
            let router = Router::new(RouterConfig::default())
                .with_provider(ModelProvider::Mock, Arc::new(mock_provider(fixtures)));
            let gw = Arc::new(
                Gateway::new(
                    router,
                    GatewayConfig {
                        run_token_budget: s.token_budget,
                        ..Default::default()
                    },
                )
                .with_audit_sink(audit_sink.clone()),
            );
            (gw.clone(), Some(gw))
        } else if let Some(url) = &s.llm_gateway_url {
            if !s.insecure_dev && s.llm_gateway_token.is_none() {
                anyhow::bail!("AMAP_LLM_GATEWAY_TOKEN is required for a remote LLM gateway");
            }
            (
                Arc::new(
                    amap_llm::GatewayClient::new(url)
                        .with_bearer_token(s.llm_gateway_token.clone()),
                ),
                None,
            )
        } else {
            let mut router = Router::new(RouterConfig::default());
            if let Some(p) = AnthropicProvider::from_env() {
                router = router.with_provider(ModelProvider::Anthropic, Arc::new(p));
            }
            if let Some(p) = OpenAiProvider::from_env() {
                router = router.with_provider(ModelProvider::OpenAi, Arc::new(p));
            }
            if let Some(p) = AzureOpenAiProvider::from_env() {
                router = router.with_provider(ModelProvider::Azure, Arc::new(p));
            }
            if router.configured().is_empty() {
                anyhow::bail!("no LLM provider configured: set ANTHROPIC_API_KEY, OPENAI_API_KEY + AMAP_OPENAI_MODEL, AZURE_OPENAI_API_KEY + AZURE_OPENAI_ENDPOINT + AZURE_OPENAI_DEPLOYMENT, AMAP_LLM_GATEWAY_URL, or run with --mock");
            }
            tracing::info!(providers = ?router.configured(), "embedded LLM gateway");
            let gw = Arc::new(
                Gateway::new(
                    router,
                    GatewayConfig {
                        run_token_budget: s.token_budget,
                        ..Default::default()
                    },
                )
                .with_audit_sink(audit_sink.clone()),
            );
            (gw.clone(), Some(gw))
        };
        Ok(Platform {
            settings: s,
            knowledge,
            lake,
            bus,
            policy,
            clock,
            ids,
            llm,
            gateway,
            mock,
        })
    }
}

impl Platform {
    /// A platform view that shares stores / bus / lake but answers LLM calls from fixtures.
    pub fn with_mock_fixtures(&self, dir: Option<&Path>) -> Platform {
        let router = Router::new(RouterConfig::default()).with_provider(
            ModelProvider::Mock,
            Arc::new(mock_provider(dir.unwrap_or(Path::new("")))),
        );
        let gw = Arc::new(
            Gateway::new(
                router,
                GatewayConfig {
                    run_token_budget: self.settings.token_budget,
                    ..Default::default()
                },
            )
            .with_audit_sink(Arc::new(KnowledgeAuditSink::new(self.knowledge.clone()))),
        );
        Platform {
            settings: self.settings.clone(),
            knowledge: self.knowledge.clone(),
            lake: self.lake.clone(),
            bus: self.bus.clone(),
            policy: self.policy.clone(),
            clock: self.clock.clone(),
            ids: self.ids.clone(),
            llm: gw.clone(),
            gateway: Some(gw),
            mock: true,
        }
    }

    /// Seed the knowledge store from a run spec and build the root agent context.
    pub async fn context_for(
        &self,
        spec: &RunSpec,
        run_id: Option<String>,
    ) -> anyhow::Result<AgentContext> {
        let function = spec.function();
        self.knowledge.upsert_function(function.clone()).await?;
        for r in spec.requirements() {
            self.knowledge
                .add_relationship(Relationship::new(
                    r.id.0.clone(),
                    RelationKind::RequirementToFunction,
                    function.id.0.clone(),
                ))
                .await?;
            self.knowledge.upsert_requirement(r).await?;
        }
        let mut config: RunConfig = spec.run.clone();
        config.token_budget = self.settings.token_budget;
        config.verifier_endpoint = self.settings.verifier_endpoint.clone();
        config.verifier_token = self.settings.worker_token.clone();
        config.verifier_tls_ca = self.settings.verifier_tls_ca.clone();
        config.verifier_tls_cert = self.settings.verifier_tls_cert.clone();
        config.verifier_tls_key = self.settings.verifier_tls_key.clone();
        config.verifier_tls_domain = self.settings.verifier_tls_domain.clone();
        config.verifier_allow_insecure = self.settings.insecure_dev;
        config.verifier_artifact_max_bytes = self.settings.worker_artifact_max_bytes;
        std::fs::create_dir_all(&config.workspace)?;
        Ok(AgentContext {
            run_id: RunId::new(run_id.unwrap_or_else(|| self.ids.next("RUN"))),
            function_id: function.id,
            knowledge: self.knowledge.clone(),
            llm: self.llm.clone(),
            bus: self.bus.clone(),
            lake: self.lake.clone(),
            policy: self.policy.clone(),
            clock: self.clock.clone(),
            ids: self.ids.clone(),
            config: Arc::new(config),
            inputs: json!({}),
        })
    }
}

/// Open the knowledge store named by the settings: PostgreSQL (with migrations) or in-memory.
pub async fn open_knowledge(settings: &Settings) -> anyhow::Result<Arc<dyn KnowledgeStore>> {
    Ok(match &settings.database_url {
        Some(url) => {
            let pg = amap_knowledge::PgKnowledgeStore::connect(url).await?;
            pg.migrate().await?;
            tracing::info!("knowledge store: PostgreSQL");
            Arc::new(pg)
        }
        None => {
            if !settings.insecure_dev {
                tracing::warn!("no database_url: knowledge, HITL reviews and the LLM audit ledger are in-memory and will not survive a restart");
            } else {
                tracing::info!("knowledge store: in-memory");
            }
            Arc::new(InMemoryKnowledgeStore::new())
        }
    })
}

fn read_fixture(dir: &Path, name: &str) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(dir.join(name)).ok()?).ok()
}

/// Fixture-driven mock: answers are read from `<fixtures>/*.json`; code fixtures are `.py` files.
/// The RCA / Fix handlers pick fixtures from the failure evidence in the prompt, so the closed loop
/// (verify → RCA → fix → review → verify) runs offline exactly as it would with real models.
pub fn mock_provider(dir: &Path) -> MockProvider {
    let d = dir.to_path_buf();
    let read_code =
        |d: &Path, name: &str| std::fs::read_to_string(d.join(name)).unwrap_or_default();
    let d1 = d.clone();
    let d2 = d.clone();
    let d3 = d.clone();
    let d4 = d.clone();
    let d5 = d.clone();
    let d6 = d.clone();
    let d7 = d.clone();
    let d8 = d.clone();
    MockProvider::labelled("fixture-mock")
        .on(TaskKind::DomainClassification, move |_| read_fixture(&d1, "domains.json").unwrap_or(json!({"domains": [], "suspicious": []})))
        .on(TaskKind::BusinessRuleExtraction, move |_| read_fixture(&d2, "rules.json").unwrap_or(json!({"rules": []})))
        .on(TaskKind::ArchitectureMapping, move |_| read_fixture(&d3, "architecture.json").unwrap_or(json!({"decision": "n/a", "evidence": ["fixture"], "alternatives": [], "risks": ["fixture"], "affected_rules": [], "affected_tests": [], "rule_ownership": []})))
        .on(TaskKind::CodeGeneration, move |_| json!({ "files": [{ "path": "loan_service.py", "content": read_code(&d4, "loan_service_v1.py") }], "notes": "initial build (fixture v1)" }))
        .on(TaskKind::TestGeneration, move |_| read_fixture(&d5, "tests.json").unwrap_or(json!({"scenarios": []})))
        .on(TaskKind::AdversarialProbe, move |_| read_fixture(&d6, "adversarial.json").unwrap_or(json!({"probes": []})))
        .on(TaskKind::RootCauseAnalysis, move |req| {
            // Choose the hypothesis that explains the differences present in the evidence.
            let p = &req.prompt;
            if p.contains("\"path\": \"output.interest\"") || p.contains("output.interest") {
                read_fixture(&d7, "rca_interest.json")
            } else {
                read_fixture(&d7, "rca_fee.json")
            }
            .unwrap_or(json!({"summary": "unknown", "location": null, "legacy_behavior": "", "next_behavior": "", "affected_rules": []}))
        })
        .on(TaskKind::PatchGeneration, move |req| {
            let p = &req.prompt;
            let (file, rationale) = if p.contains("365.25") {
                ("loan_service_fix1.py", "Use ACT/365 (DAYS_IN_YEAR = 365) as in LOAN231 3000-CALC-INTEREST")
            } else {
                ("loan_service_fix2.py", "Apply the 1,000 won fee floor from LOAN231 4000-CALC-FEE")
            };
            json!({ "rationale": rationale, "files": [{ "path": "loan_service.py", "content": read_code(&d8, file) }] })
        })
        .on(TaskKind::CodeReview, move |_| read_fixture(&d, "review.json").unwrap_or(json!({"approved": true, "comments": []})))
        .on_fixture(TaskKind::BusinessReview, json!({"approved": true, "comments": []}))
        .on_fixture(TaskKind::BehaviorLinking, json!({"links": []}))
}
