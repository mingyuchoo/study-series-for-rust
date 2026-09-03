# AMAP — Autonomous Modernization Assurance Platform

Rust-first implementation of the design in [`docs/init.md`](docs/init.md): an **evidence-driven,
closed-loop legacy modernization system** whose accuracy comes from production behavior capture,
a functional-equivalence harness, a business-rule knowledge graph, an uncertainty engine and an
evidence ledger — with LLMs used as swappable execution engines that never make the final PASS/FAIL call.

```
Control Plane (Axum/Cedar) ─► Orchestrator (DAG, NATS) ─► Agents ─► Verification Factory ─► Quality Gate
                                   │                                    │
                          Knowledge Graph (PostgreSQL / petgraph)     Evidence Lake (Parquet / DataFusion)
```

## Quick start (offline, no API keys)

```bash
cargo run -q --bin amap -- demo          # closed-loop run on examples/loan-demo with fixture-driven LLM answers
cargo test --workspace                   # unit tests
make wasm                                # build the WASM comparator plugin (plugins/finance)
```

The demo modernizes a COBOL early-repayment program (`examples/loan-demo/legacy/LOAN231.cbl`):
discovery → rule mining → behavior mining (300 production traces → golden master) → uncertainty/HITL →
architecture → build → independent tests / boundary / adversarial → verification → RCA → fix → review → re-verify.
The first build carries two defects (ACT/365.25 day-count, missing fee floor); the loop finds and repairs
both and ends with **FE CERTIFIED** on all ten gate KPIs (mutation score 100%, unexplained differences 0).

## Repository layout (stack §20)

| Path | Role |
|---|---|
| `crates/domain` | Canonical model: rules, behavior records, scenarios, verification results, evidence, gate |
| `crates/comparator` | Functional Equivalence Engine: exact/numeric/tolerance/format/unordered/ignore + **Wasmtime** plugins, YAML specs |
| `crates/invariant` | Business Invariant DSL (pest) → AST → evaluator; also used for rule conditions (`WHEN … THEN …`) |
| `crates/uncertainty` | Evidence-based confidence scoring, `U = f(R,B,T,D,E,C)`, HITL tiers, residual uncertainty, N-version agreement |
| `crates/policy` | Agent governance with **Cedar** (no self-approval, independent verifier, LLM never decides PASS/FAIL) |
| `crates/knowledge` | Knowledge store: PostgreSQL (SQLx, JSONB + pgvector migration) and in-memory |
| `crates/graph` | petgraph projection: impact / lineage / weakly-evidenced rules / DOT |
| `crates/evidence` | Evidence lake: Parquet on local FS or S3/MinIO, queried in-process with **DataFusion**; certificate builder |
| `crates/context` | Context Engine: Tantivy lexical + vector retrieval → bounded, PII-masked `ContextPack` |
| `crates/replay` | Golden replay / differential / state / invariant engine; JSON-lines subprocess and HTTP SUT adapters |
| `crates/llm` | LLM gateway core: Anthropic (raw Messages API) / OpenAI-compatible / mock providers, role routing, PII filter, cache, retries, budgets, audit, cost |
| `crates/orchestrator` | `AgentTask` abstraction, DAG executor with retries + HITL halts, in-memory/NATS bus, gRPC proto (Tonic) |
| `crates/agents` | Discovery (COBOL analyzer + tree-sitter), Rule Miner, Behavior Miner, Uncertainty, Architecture, Builder, Test Generator, Boundary, Adversarial, Verifier (static/golden/differential/property/mutation/fault/concurrency), RCA, Fix, Review, closed-loop workflow |
| `services/control-plane` | REST API: runs, functions, rules/behaviors/scenarios/evidence, HITL reviews, events, evidence SQL, graph, metrics |
| `services/llm-gateway` | Standalone gateway (`POST /v1/complete`, `/v1/audit`) |
| `services/amap-cli` | `amap` CLI + shared bootstrap/run-spec code |
| `workers/` | gRPC verification workers (`verification-worker`, `replay-`, `mutation-`, `fault-`, `concurrency-`, `comparator-worker`) |
| `plugins/finance` | Example WASM comparator plugin |
| `proto/`, `migrations/`, `config/`, `deploy/`, `docker-compose.yml` | Wire contract, PostgreSQL schema, settings, Kubernetes/Prometheus, local infra |

## Running for real

1. Infrastructure: `docker compose up -d` (PostgreSQL+pgvector, NATS JetStream, Redpanda, MinIO, Prometheus, Grafana).
2. Settings: `config/amap.toml` or `AMAP_*` env (`AMAP_DATABASE_URL`, `AMAP_NATS_URL`, `AMAP_LAKE=s3://…`, `AMAP_LLM_GATEWAY_URL`).
3. Models: `ANTHROPIC_API_KEY` (default model `claude-opus-5`, adaptive thinking, server-side refusal fallbacks) and/or
   `OPENAI_API_KEY` + `AMAP_OPENAI_MODEL` (set to the engagement's deployed model id). Builder and reviewer/adversarial
   are routed to different providers when both are configured (design §20/§26).
4. Services: `cargo run --bin llm-gateway`, `cargo run --bin control-plane`, `cargo run --bin verification-worker`.
5. Start a run: `curl -X POST localhost:8080/v1/runs -d '{"spec":"/abs/path/amap.toml"}'`, poll `/v1/runs/{id}`,
   decide HITL reviews via `POST /v1/reviews/{id}/decide`. Set `verifier_endpoint` in the run spec to push
   verification to the worker pool over gRPC.

Write a run spec like `examples/loan-demo/amap.toml`: legacy source root, documents, production traces (JSON lines),
next-system command (JSON-lines protocol), optional legacy oracle command, comparator specs, invariants,
faults, a concurrency template and gate thresholds.

## CLI

```
amap run --spec amap.toml [--mock]      amap compare --spec spec.yaml [--plugin name=x.wasm] expected.json actual.json
amap demo                               amap invariant rules.dsl data.json
amap evidence --sql "SELECT …"          amap gate certificate.json
amap graph snapshot.json > graph.dot    amap discover legacy/
```

## Design → code map

| Design element | Where |
|---|---|
| Rule confidence table (0.99 / 0.95 / 0.85 / 0.65 / 0.40) and evidence adjustments | `amap_domain::EvidenceSources::baseline_confidence`, `amap_uncertainty::rule_confidence` |
| Golden Master from production behaviors | `amap_replay::scenarios_from_behaviors`, Behavior Miner |
| Field-level equivalence policy (EXACT / ±2s / FORMAT+UNIQUE / ORDER-INSENSITIVE / IGNORE) | `crates/comparator/src/spec.rs`, `examples/loan-demo/specs/loan-repayment.yaml` |
| Business invariants that catch "both systems wrong" | `crates/invariant`, `examples/loan-demo/specs/invariants.dsl` |
| Boundary generation (64/65/66 …) | `crates/agents/src/boundary.rs` |
| Mutation detection rate KPI | `crates/agents/src/verification/mutation.rs` |
| Fault / concurrency verification | `verification/fault.rs`, `verification/concurrency.rs` |
| RCA → Fix → Review separation, model diversification | `rca.rs`, `fix.rs`, `review.rs`, `amap_llm::RouterConfig`, Cedar policies |
| Risk-based HITL (>0.98 auto … <0.70 SME) | `amap_domain::HitlTier`, Uncertainty agent, `/v1/reviews` |
| Evidence ledger + function certificate | `crates/evidence`, `FunctionCertificate` |
| Quality gate (Critical 100%, P0 100%, P1 ≥99.999%, FE ≥99.9%, mutation ≥99%, unexplained = 0, residual ≤0.1%) | `amap_domain::evaluate_gate` |

## Known gaps / next steps

- Durable execution: the DAG executor is in-process; swap in Temporal (Rust SDK is still public preview) behind `AgentTask`.
- Data plane: Redpanda is provisioned but production capture/CDC ingestion into the lake is not wired (traces are read from files).
- Semantic retrieval uses hashed bag-of-words vectors; plug real embeddings into `VectorIndex` / the `embeddings` table.
- OpenTelemetry export sits behind the `tracing` adapter boundary; the exporter is not compiled in (OTel-Rust is beta).
- Mutation operators are textual; equivalent-mutant detection is left to scenario design.
