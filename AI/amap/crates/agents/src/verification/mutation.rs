//! Mutation Agent (design §15): inject synthetic defects, check the test system catches them.
use super::build_engine;
use amap_domain::*;
use amap_orchestrator::RunConfig;
use amap_replay::{ExecOptions, ProcessSystem};
use serde_json::{json, Value};
use walkdir::WalkDir;

/// Textual mutation operators (from → to). Each occurrence yields one mutant.
pub const OPERATORS: &[(&str, &str)] = &[
    (">=", ">"),
    ("<=", "<"),
    ("> ", ">= "),
    ("< ", "<= "),
    ("== ", "!= "),
    ("365", "360"),
    ("/ 365", "/ 365.25"),
    ("- fee", "+ fee"),
    ("* 100", "* 10"),
    ("round(", "int("),
    ("ROUND(", "TRUNC("),
    ("0.015", "0.02"),
    ("0.005", "0.01"),
    ("1000", "0"),
];

pub struct Mutant {
    pub file: String,
    pub description: String,
    pub content: String,
}

pub fn generate_mutants(workspace: &std::path::Path, cap: usize) -> Vec<Mutant> {
    let mut mutants = Vec::new();
    let files: Vec<(String, String)> = WalkDir::new(workspace)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let rel = e.path().strip_prefix(workspace).ok()?.display().to_string();
            if rel.starts_with(".amap") || !matches!(Language::from_path(&rel), Language::Python | Language::Java | Language::Rust) {
                return None;
            }
            Some((rel, std::fs::read_to_string(e.path()).ok()?))
        })
        .collect();
    for (rel, text) in &files {
        for (from, to) in OPERATORS {
            let mut start = 0;
            while let Some(pos) = text[start..].find(from) {
                let abs = start + pos;
                // skip comments
                let line_start = text[..abs].rfind('\n').map(|i| i + 1).unwrap_or(0);
                let line_end = text[abs..].find('\n').map(|i| abs + i).unwrap_or(text.len());
                let line = &text[line_start..line_end];
                let trimmed = line.trim_start();
                let is_doc = trimmed.starts_with('#') || trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') || line.contains("\"\"\"") || line.contains("'''");
                if !is_doc {
                    let mut mutated = String::with_capacity(text.len());
                    mutated.push_str(&text[..abs]);
                    mutated.push_str(to);
                    mutated.push_str(&text[abs + from.len()..]);
                    let line_no = text[..abs].matches('\n').count() + 1;
                    mutants.push(Mutant { file: rel.clone(), description: format!("{rel}:{line_no} `{from}` → `{to}`"), content: mutated });
                }
                start = abs + from.len();
            }
        }
    }
    if mutants.len() > cap {
        // spread evenly
        let step = mutants.len() as f64 / cap as f64;
        let mut picked = Vec::new();
        let mut i = 0.0;
        while picked.len() < cap && (i as usize) < mutants.len() {
            picked.push(mutants.remove(i as usize));
            i += step - 1.0;
        }
        mutants = picked;
    }
    mutants
}

pub async fn run(scenarios: &[TestScenario], cfg: &RunConfig, function_id: &FunctionId) -> Result<(Vec<VerificationResult>, Value), String> {
    let with_expected: Vec<TestScenario> = scenarios.iter().filter(|s| s.expected_output.is_some()).cloned().collect();
    if with_expected.is_empty() {
        return Ok((vec![], json!({ "injected": 0, "detected": 0, "reason": "no scenarios with expectations" })));
    }
    let engine = build_engine(cfg)?;
    let mutants = generate_mutants(&cfg.workspace, cfg.max_mutants);
    let mut results = Vec::new();
    let mut survivors = Vec::new();
    let sandbox_root = cfg.workspace.join(".amap-mutants");
    let _ = std::fs::remove_dir_all(&sandbox_root);
    for (i, m) in mutants.iter().enumerate() {
        let dir = sandbox_root.join(format!("m{i}"));
        copy_workspace(&cfg.workspace, &dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(&m.file), &m.content).map_err(|e| e.to_string())?;
        let cmd: Vec<String> = cfg.next_command.iter().map(|c| c.replace("{workspace}", &dir.display().to_string())).collect();
        let (program, args) = cmd.split_first().ok_or("next_command empty")?;
        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let sut = ProcessSystem::new("mutant", program, &args);
        let res = engine.replay(&sut, None, &with_expected, VerificationKind::Mutation, &ExecOptions::default()).await.map_err(|e| e.to_string())?;
        let mut detected = res.iter().any(|r| !r.passed);
        if !detected && cfg.concurrency_template.is_some() {
            // Concurrency semantics are part of the killable surface too.
            let mut cfg2 = cfg.clone();
            cfg2.workspace = dir.clone();
            if let Ok((conc, _)) = super::concurrency::run(&cfg2, function_id).await {
                detected = conc.iter().any(|r| !r.passed);
            }
        }
        if !detected {
            survivors.push(m.description.clone());
        }
        results.push(VerificationResult {
            kind: VerificationKind::Mutation,
            scenario_id: ScenarioId::new(format!("MUT-{i:04}")),
            function_id: function_id.clone(),
            rule_ids: vec![],
            priority: Priority::P1,
            passed: detected,
            differences: if detected { vec![] } else { vec![Difference { path: "$".into(), expected: json!("mutant detected"), actual: json!("survived"), comparator: "mutation".into(), message: m.description.clone() }] },
            explained: false,
            explanation: None,
            duration_ms: 0,
            details: json!({ "mutant": m.description, "killed_by": res.iter().filter(|r| !r.passed).map(|r| r.scenario_id.0.clone()).take(3).collect::<Vec<_>>() }),
        });
    }
    let _ = std::fs::remove_dir_all(&sandbox_root);
    let detected = results.iter().filter(|r| r.passed).count();
    Ok((results, json!({ "injected": mutants.len(), "detected": detected, "survivors": survivors })))
}

fn copy_workspace(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let rel = entry.path().strip_prefix(src).unwrap();
        if rel.starts_with(".amap-mutants") || rel.as_os_str().is_empty() {
            continue;
        }
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(p) = target.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
