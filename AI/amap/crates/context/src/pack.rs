use amap_domain::*;
use crate::Hit;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodeSnippet {
    pub location: SourceLocation,
    pub symbol: String,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ContextBudget {
    pub max_tokens: usize,
    pub max_behaviors: usize,
    pub max_tests: usize,
    pub max_search_hits: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self { max_tokens: 60_000, max_behaviors: 12, max_tests: 12, max_search_hits: 10 }
    }
}

/// Everything an agent needs for one bounded context — and nothing else.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextPack {
    pub function: Option<BusinessFunction>,
    pub requirements: Vec<Requirement>,
    pub rules: Vec<BusinessRule>,
    pub source: Vec<CodeSnippet>,
    pub schema: Vec<DbEntity>,
    pub interfaces: Vec<InterfaceSpec>,
    pub behaviors: Vec<BehaviorRecord>,
    pub tests: Vec<TestScenario>,
    pub decisions: Vec<ArchitectureDecision>,
    pub related: Vec<Hit>,
    pub truncated: bool,
    pub estimated_tokens: usize,
}

pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

impl ContextPack {
    /// Render as Markdown for a prompt.
    pub fn render(&self) -> String {
        let mut s = String::new();
        if let Some(f) = &self.function {
            s.push_str(&format!("# Function {} — {} ({} / {:?})\n{}\n\n", f.id, f.name, f.domain, f.priority, f.description));
        }
        if !self.requirements.is_empty() {
            s.push_str("## Requirements\n");
            for r in &self.requirements {
                s.push_str(&format!("- {} {}: {}\n", r.id, r.title, r.text));
            }
            s.push('\n');
        }
        if !self.rules.is_empty() {
            s.push_str("## Business rules\n");
            for r in &self.rules {
                s.push_str(&format!(
                    "- {} [{:?}, confidence {:.2}] {}\n  WHEN {} THEN {}\n  source: {}\n",
                    r.id,
                    r.priority,
                    r.confidence,
                    r.name,
                    r.condition,
                    r.result,
                    r.sources.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(", ")
                ));
            }
            s.push('\n');
        }
        if !self.schema.is_empty() {
            s.push_str("## Database entities\n");
            for d in &self.schema {
                s.push_str(&format!("- {} {}{}\n", d.id, d.table, d.column.as_ref().map(|c| format!(".{c}")).unwrap_or_default()));
            }
            s.push('\n');
        }
        if !self.interfaces.is_empty() {
            s.push_str("## Interfaces\n");
            for i in &self.interfaces {
                s.push_str(&format!("- {} {} ({} {})\n", i.id, i.name, i.kind, i.direction));
            }
            s.push('\n');
        }
        if !self.decisions.is_empty() {
            s.push_str("## Architecture constraints\n");
            for d in &self.decisions {
                s.push_str(&format!("- {}: {}\n", d.id, d.decision));
            }
            s.push('\n');
        }
        if !self.source.is_empty() {
            s.push_str("## Legacy source\n");
            for c in &self.source {
                s.push_str(&format!("### {} ({})\n```\n{}\n```\n", c.symbol, c.location, c.text));
            }
            s.push('\n');
        }
        if !self.behaviors.is_empty() {
            s.push_str("## Production behaviors (golden)\n");
            for b in &self.behaviors {
                s.push_str(&format!("- {} input={} legacy_output={}\n", b.id, b.input, b.legacy_output));
            }
            s.push('\n');
        }
        if !self.tests.is_empty() {
            s.push_str("## Golden tests\n");
            for t in &self.tests {
                s.push_str(&format!("- {} [{:?}] input={} expected={}\n", t.id, t.origin, t.input, t.expected_output.clone().unwrap_or_default()));
            }
            s.push('\n');
        }
        if !self.related.is_empty() {
            s.push_str("## Related knowledge (search)\n");
            for h in &self.related {
                s.push_str(&format!("- {} ({}) score {:.2}\n", h.id, h.kind, h.score));
            }
        }
        s
    }

    /// Trim the pack until it fits the token budget (drops lowest-value sections first).
    pub fn fit(&mut self, max_tokens: usize) {
        self.estimated_tokens = estimate_tokens(&self.render());
        while self.estimated_tokens > max_tokens {
            self.truncated = true;
            if !self.related.is_empty() {
                self.related.pop();
            } else if self.tests.len() > 3 {
                self.tests.pop();
            } else if self.behaviors.len() > 3 {
                self.behaviors.pop();
            } else if let Some(last) = self.source.last_mut() {
                if last.text.len() > 400 {
                    last.text.truncate(400);
                    last.text.push_str("\n… (truncated)");
                } else {
                    self.source.pop();
                }
            } else {
                break;
            }
            self.estimated_tokens = estimate_tokens(&self.render());
        }
    }
}
