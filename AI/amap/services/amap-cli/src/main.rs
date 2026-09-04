//! `amap` — command-line entry point for the Autonomous Modernization Assurance Platform.
use amap_cli::{PlatformBuilder, RunSpec, Settings};
use amap_domain::*;
use anyhow::Context;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "amap",
    version,
    about = "Autonomous Modernization Assurance Platform"
)]
struct Cli {
    /// Platform settings file (defaults to config/amap.toml; AMAP_* env vars override)
    #[arg(long, global = true)]
    settings: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the closed-loop modernization workflow for a run spec (amap.toml)
    Run {
        #[arg(long, default_value = "amap.toml")]
        spec: PathBuf,
        /// Use fixture-driven mock LLM answers (offline)
        #[arg(long)]
        mock: bool,
        /// Write the full JSON outcome here
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Run the bundled loan-repayment demo offline (equivalent to `run --mock --spec examples/loan-demo/amap.toml`)
    Demo {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Compare two JSON documents under a comparator spec
    Compare {
        #[arg(long)]
        spec: Option<PathBuf>,
        /// WASM comparator plugins as name=path.wasm
        #[arg(long)]
        plugin: Vec<String>,
        expected: PathBuf,
        actual: PathBuf,
    },
    /// Check business invariants (DSL file) against a JSON document
    Invariant { rules: PathBuf, data: PathBuf },
    /// Evaluate the quality gate for a certificate JSON (+ optional equivalence metrics JSON)
    Gate {
        certificate: PathBuf,
        #[arg(long)]
        metrics: Option<PathBuf>,
    },
    /// Query the evidence lake with SQL (table: evidence)
    Evidence {
        #[arg(
            long,
            default_value = "SELECT function_id, kind, priority, count(*) AS total, sum(CASE WHEN passed THEN 1 ELSE 0 END) AS passed FROM evidence GROUP BY function_id, kind, priority ORDER BY 1,2,3"
        )]
        sql: String,
    },
    /// Render the knowledge graph (Graphviz DOT) from a snapshot JSON written by `run`
    Graph { snapshot: PathBuf },
    /// Analyse a legacy source tree (deterministic discovery only)
    Discover { root: PathBuf },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let settings = Settings::load(cli.settings.as_deref())?;
    amap_telemetry::init(&amap_telemetry::TelemetryConfig {
        service_name: "amap-cli".into(),
        json: settings.json_logs,
        otlp_endpoint: settings.otlp_endpoint.clone(),
    });
    match cli.cmd {
        Cmd::Run { spec, mock, out } => run(settings, spec, mock, out).await,
        Cmd::Demo { out } => {
            let spec = std::env::current_dir()?.join("examples/loan-demo/amap.toml");
            run(settings, spec, true, out).await
        }
        Cmd::Compare {
            spec,
            plugin,
            expected,
            actual,
        } => {
            let spec = match spec {
                Some(p) => {
                    amap_comparator::ComparatorSpec::from_yaml(&std::fs::read_to_string(p)?)?
                }
                None => amap_comparator::ComparatorSpec::exact("exact"),
            };
            let mut engine = amap_comparator::ComparisonEngine::new();
            for p in plugin {
                let (name, path) = p.split_once('=').context("plugin must be name=path.wasm")?;
                engine.register_plugin(
                    name,
                    std::sync::Arc::new(amap_comparator::wasm::WasmComparator::from_file(
                        name, path,
                    )?),
                );
            }
            let e: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(expected)?)?;
            let a: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(actual)?)?;
            let r = engine.compare(&spec, &e, &a)?;
            println!("{}", serde_json::to_string_pretty(&r)?);
            if !r.equal {
                std::process::exit(1);
            }
            Ok(())
        }
        Cmd::Invariant { rules, data } => {
            let set = amap_invariant::parse(&std::fs::read_to_string(rules)?)?;
            let d: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(data)?)?;
            let out = amap_invariant::check_all(&set, &d);
            println!("{}", serde_json::to_string_pretty(&out)?);
            if out.iter().any(|o| o.applicable && !o.holds) {
                std::process::exit(1);
            }
            Ok(())
        }
        Cmd::Gate {
            certificate,
            metrics,
        } => {
            let cert: FunctionCertificate =
                serde_json::from_str(&std::fs::read_to_string(certificate)?)?;
            let eq: EquivalenceMetrics = match metrics {
                Some(m) => serde_json::from_str(&std::fs::read_to_string(m)?)?,
                None => EquivalenceMetrics {
                    all_total: cert.golden.total,
                    all_passed: cert.golden.passed,
                    ..Default::default()
                },
            };
            let report = evaluate_gate(&cert, &eq, &QualityGateThresholds::default());
            print_gate(&report);
            if !report.certified {
                std::process::exit(1);
            }
            Ok(())
        }
        Cmd::Evidence { sql } => {
            let lake = if settings.lake.starts_with("s3://") {
                amap_evidence::EvidenceLake::open_s3(&settings.lake).await?
            } else {
                amap_evidence::EvidenceLake::open_local(&settings.lake).await?
            };
            let rows = lake.query(&sql).await?;
            println!("{}", serde_json::to_string_pretty(&rows)?);
            Ok(())
        }
        Cmd::Graph { snapshot } => {
            let snap: amap_knowledge::KnowledgeSnapshot =
                serde_json::from_str(&std::fs::read_to_string(snapshot)?)?;
            let g = amap_graph::KnowledgeGraph::from_snapshot(&snap);
            print!("{}", g.to_dot());
            Ok(())
        }
        Cmd::Discover { root } => {
            for (file, a) in amap_agents::discovery::analyze_tree(&root) {
                println!(
                    "== {file}: {} entities, {} db entities, {} relationships",
                    a.entities.len(),
                    a.db_entities.len(),
                    a.relationships.len()
                );
                for e in a.entities {
                    println!(
                        "  {:?} {} @ {} -> {}",
                        e.kind,
                        e.symbol,
                        e.location,
                        e.dependencies
                            .iter()
                            .map(|d| d.0.rsplit("::").next().unwrap_or("").to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
            Ok(())
        }
    }
}

async fn run(
    settings: Settings,
    spec_path: PathBuf,
    mock: bool,
    out: Option<PathBuf>,
) -> anyhow::Result<()> {
    let spec =
        RunSpec::load(&spec_path).with_context(|| format!("loading {}", spec_path.display()))?;
    let mut builder = PlatformBuilder::new(settings);
    if mock {
        builder = builder.with_mock_fixtures(spec.mock.fixtures.as_deref());
    }
    let platform = builder.build().await?;
    let ctx = platform.context_for(&spec, None).await?;
    println!(
        "▶ AMAP run {} for {} ({}){}",
        ctx.run_id,
        spec.function.id,
        spec.function.name,
        if platform.mock { " [mock LLM]" } else { "" }
    );
    let outcome = amap_agents::run_modernization(ctx.clone()).await?;

    println!("\n== Steps ==");
    for (i, r) in outcome.reports.iter().enumerate() {
        for s in &r.steps {
            println!(
                "  [{i}] {:<16} {:<9} {:>6}ms  {}",
                s.name,
                format!("{:?}", s.status),
                s.duration_ms,
                s.summary
            );
        }
    }
    if let Some(g) = &outcome.gate {
        println!();
        print_gate(g);
    }
    if let Some(gw) = &platform.gateway {
        let audit = gw.audit_log();
        let cost: f64 = audit.iter().map(|a| a.cost_usd).sum();
        let tokens: u64 = audit.iter().map(|a| a.input_tokens + a.output_tokens).sum();
        println!(
            "\n== LLM audit == {} calls, {} tokens, ${:.4}, {} cached",
            audit.len(),
            tokens,
            cost,
            audit.iter().filter(|a| a.cached).count()
        );
    }
    let snapshot = platform.knowledge.snapshot().await?;
    let lake_dir = PathBuf::from(&platform.settings.lake);
    if !platform.settings.lake.starts_with("s3://") {
        std::fs::create_dir_all(&lake_dir)?;
        std::fs::write(
            lake_dir.join(format!("{}-snapshot.json", ctx.run_id)),
            serde_json::to_string_pretty(&snapshot)?,
        )?;
        println!(
            "knowledge snapshot: {}",
            lake_dir
                .join(format!("{}-snapshot.json", ctx.run_id))
                .display()
        );
    }
    println!(
        "\n== Result == {} after {} repair iteration(s){}{}",
        if outcome.certified {
            "FE CERTIFIED ✅"
        } else {
            "NOT CERTIFIED ❌"
        },
        outcome.iterations,
        outcome
            .halted
            .as_ref()
            .map(|h| format!(" — halted: {h}"))
            .unwrap_or_default(),
        outcome
            .failed
            .as_ref()
            .map(|f| format!(" — failed: {f}"))
            .unwrap_or_default()
    );
    if let Some(o) = out {
        std::fs::write(&o, serde_json::to_string_pretty(&outcome)?)?;
        println!("outcome written to {}", o.display());
    }
    if !outcome.certified {
        std::process::exit(2);
    }
    Ok(())
}

fn print_gate(g: &GateReport) {
    println!(
        "== Quality Gate for {} → {} ==",
        g.function_id,
        if g.certified { "CERTIFIED" } else { "BLOCKED" }
    );
    for c in &g.checks {
        println!(
            "  {} {:<34} required {:<12} actual {}",
            if c.passed { "✔" } else { "✘" },
            c.kpi,
            c.required,
            c.actual
        );
    }
}
