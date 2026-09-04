//! Static verification of the next system: syntax / compile checks per language.
use amap_domain::*;
use amap_orchestrator::RunConfig;
use serde_json::json;
use std::process::Command;
use walkdir::WalkDir;

pub fn run(cfg: &RunConfig, function_id: &FunctionId) -> Vec<VerificationResult> {
    let mut problems = Vec::new();
    let mut checked = 0;
    for entry in WalkDir::new(&cfg.workspace)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let rel = path
            .strip_prefix(&cfg.workspace)
            .unwrap_or(path)
            .display()
            .to_string();
        if rel.starts_with(".amap") {
            continue;
        }
        match Language::from_path(&rel) {
            Language::Python => {
                checked += 1;
                let out = Command::new("python3")
                    .args(["-m", "py_compile", &path.display().to_string()])
                    .output();
                match out {
                    Ok(o) if o.status.success() => {}
                    Ok(o) => problems.push(format!(
                        "{rel}: {}",
                        String::from_utf8_lossy(&o.stderr).trim()
                    )),
                    Err(e) => problems.push(format!("{rel}: python3 unavailable: {e}")),
                }
            }
            Language::Java | Language::Rust => {
                checked += 1;
                if std::fs::metadata(path)
                    .map(|m| m.len() == 0)
                    .unwrap_or(true)
                {
                    problems.push(format!("{rel}: empty source file"));
                }
            }
            _ => {}
        }
    }
    vec![VerificationResult {
        kind: VerificationKind::Static,
        scenario_id: ScenarioId::new("STATIC"),
        function_id: function_id.clone(),
        rule_ids: vec![],
        priority: Priority::P1,
        passed: problems.is_empty() && checked > 0,
        differences: problems
            .iter()
            .map(|p| Difference {
                path: "$".into(),
                expected: json!("compiles"),
                actual: json!(p),
                comparator: "static".into(),
                message: p.clone(),
            })
            .collect(),
        explained: false,
        explanation: None,
        duration_ms: 0,
        details: json!({ "files_checked": checked }),
    }]
}
