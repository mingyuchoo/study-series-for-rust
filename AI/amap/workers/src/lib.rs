//! Verification workers: gRPC (Tonic) services that run one or more verification engines.
//! The orchestrator dispatches to them when `verifier_endpoint` is configured.
use amap_domain::*;
use amap_orchestrator::proto::verification_server::{Verification, VerificationServer};
use amap_orchestrator::proto::{HealthRequest, HealthResponse, VerifyRequest, VerifyResponse};
use amap_orchestrator::RunConfig;
use tonic::{Request, Response, Status};

pub struct Worker {
    pub name: String,
    pub kinds: Vec<VerificationKind>,
}

fn kind_name(k: VerificationKind) -> String {
    serde_json::to_value(k).unwrap().as_str().unwrap().to_string()
}

#[tonic::async_trait]
impl Verification for Worker {
    async fn verify(&self, req: Request<VerifyRequest>) -> Result<Response<VerifyResponse>, Status> {
        let req = req.into_inner();
        let kind: VerificationKind = serde_json::from_value(serde_json::Value::String(req.kind.clone())).map_err(|e| Status::invalid_argument(e.to_string()))?;
        if !self.kinds.contains(&kind) {
            return Err(Status::failed_precondition(format!("{} does not serve {}", self.name, req.kind)));
        }
        let scenarios: Vec<TestScenario> = serde_json::from_slice(&req.scenarios_json).map_err(|e| Status::invalid_argument(e.to_string()))?;
        let config: RunConfig = serde_json::from_slice(&req.config_json).map_err(|e| Status::invalid_argument(e.to_string()))?;
        let function_id = FunctionId::new(req.function_id);
        amap_telemetry::Metrics::global().inc("amap_worker_requests_total", &[("kind", &req.kind)]);
        tracing::info!(run = %req.run_id, function = %function_id, kind = %req.kind, scenarios = scenarios.len(), "verify request");
        let (results, metrics) = amap_agents::verification::run_engine(kind, &scenarios, &config, &function_id).await.map_err(Status::internal)?;
        tracing::info!(kind = %req.kind, results = results.len(), passed = results.iter().filter(|r| r.passed).count(), "verify done");
        Ok(Response::new(VerifyResponse { results_json: serde_json::to_vec(&results).unwrap(), metrics_json: serde_json::to_vec(&metrics).unwrap(), worker: self.name.clone() }))
    }

    async fn health(&self, _: Request<HealthRequest>) -> Result<Response<HealthResponse>, Status> {
        Ok(Response::new(HealthResponse { status: "ok".into(), worker: self.name.clone(), kinds: self.kinds.iter().map(|k| kind_name(*k)).collect() }))
    }
}

pub async fn serve(name: &str, kinds: Vec<VerificationKind>, listen: Option<String>) -> anyhow::Result<()> {
    let settings = amap_cli::Settings::load(None)?;
    amap_telemetry::init(&amap_telemetry::TelemetryConfig { service_name: name.into(), json: settings.json_logs, otlp_endpoint: settings.otlp_endpoint.clone() });
    let addr = listen.unwrap_or(settings.worker_listen).parse()?;
    tracing::info!(%addr, kinds = ?kinds, "{name} listening");
    tonic::transport::Server::builder().add_service(VerificationServer::new(Worker { name: name.into(), kinds })).serve(addr).await?;
    Ok(())
}
